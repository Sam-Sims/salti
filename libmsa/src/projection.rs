use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) enum Projection {
    Full { len: usize },
    Filtered(Arc<[usize]>),
}

pub(crate) enum ProjectionIter<'a> {
    Full(std::ops::Range<usize>),
    Filtered(std::slice::Iter<'a, usize>),
}

impl Projection {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Full { len } => *len,
            Self::Filtered(ids) => ids.len(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn absolute(&self, relative: usize) -> Option<usize> {
        match self {
            Self::Full { len } => (relative < *len).then_some(relative),
            Self::Filtered(ids) => ids.get(relative).copied(),
        }
    }

    pub(crate) fn relative(&self, absolute: usize) -> Option<usize> {
        match self {
            Self::Full { len } => (absolute < *len).then_some(absolute),
            Self::Filtered(ids) => ids.binary_search(&absolute).ok(),
        }
    }

    pub(crate) fn iter(&self) -> ProjectionIter<'_> {
        match self {
            Self::Full { len } => ProjectionIter::Full(0..*len),
            Self::Filtered(ids) => ProjectionIter::Filtered(ids.iter()),
        }
    }

    pub(crate) fn is_full(&self) -> bool {
        matches!(self, Self::Full { .. })
    }
}

impl Iterator for ProjectionIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Full(range) => range.next(),
            Self::Filtered(iter) => iter.next().copied(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Full(range) => range.size_hint(),
            Self::Filtered(iter) => iter.size_hint(),
        }
    }
}

impl ExactSizeIterator for ProjectionIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_projection_basics() {
        let proj = Projection::Full { len: 5 };
        assert_eq!(proj.len(), 5);
        assert!(!proj.is_empty());
        assert!(proj.is_full());
        assert_eq!(proj.absolute(0), Some(0));
        assert_eq!(proj.absolute(4), Some(4));
        assert_eq!(proj.absolute(5), None);
    }

    #[test]
    fn filtered_projection_basics() {
        let proj = Projection::Filtered(Arc::from([1, 3, 7].as_slice()));
        assert_eq!(proj.len(), 3);
        assert!(!proj.is_empty());
        assert!(!proj.is_full());
        assert_eq!(proj.absolute(0), Some(1));
        assert_eq!(proj.absolute(1), Some(3));
        assert_eq!(proj.absolute(2), Some(7));
        assert_eq!(proj.absolute(3), None);
    }

    #[test]
    fn empty_projections() {
        let full = Projection::Full { len: 0 };
        assert!(full.is_empty());
        assert_eq!(full.iter().count(), 0);

        let filtered = Projection::Filtered(Arc::from([].as_slice()));
        assert!(filtered.is_empty());
        assert_eq!(filtered.iter().count(), 0);
    }
    #[test]
    fn full_iter_yields_all_indices() {
        let proj = Projection::Full { len: 4 };
        let indices: Vec<_> = proj.iter().collect();
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn filtered_iter_yields_stored_indices() {
        let proj = Projection::Filtered(Arc::from([2, 5, 8].as_slice()));
        let indices: Vec<_> = proj.iter().collect();
        assert_eq!(indices, vec![2, 5, 8]);
    }

    #[test]
    fn exact_size_iterator() {
        let proj = Projection::Full { len: 3 };
        let iter = proj.iter();
        assert_eq!(iter.len(), 3);

        let proj = Projection::Filtered(Arc::from([1, 4].as_slice()));
        let iter = proj.iter();
        assert_eq!(iter.len(), 2);
    }

    #[test]
    fn full_projection_relative() {
        let proj = Projection::Full { len: 5 };
        assert_eq!(proj.relative(0), Some(0));
        assert_eq!(proj.relative(4), Some(4));
        assert_eq!(proj.relative(5), None);
    }

    #[test]
    fn filtered_projection_relative() {
        let proj = Projection::Filtered(Arc::from([1, 3, 7].as_slice()));
        assert_eq!(proj.relative(1), Some(0));
        assert_eq!(proj.relative(3), Some(1));
        assert_eq!(proj.relative(7), Some(2));
        assert_eq!(proj.relative(0), None);
        assert_eq!(proj.relative(2), None);
        assert_eq!(proj.relative(99), None);
    }

    #[test]
    fn empty_projection_relative() {
        let full = Projection::Full { len: 0 };
        assert_eq!(full.relative(0), None);

        let filtered = Projection::Filtered(Arc::from([].as_slice()));
        assert_eq!(filtered.relative(0), None);
    }

    #[test]
    fn relative_absolute_round_trip() {
        let proj = Projection::Filtered(Arc::from([2, 5, 8].as_slice()));
        for rel in 0..proj.len() {
            let abs = proj.absolute(rel).unwrap();
            assert_eq!(proj.relative(abs), Some(rel));
        }

        let full = Projection::Full { len: 4 };
        for rel in 0..full.len() {
            let abs = full.absolute(rel).unwrap();
            assert_eq!(full.relative(abs), Some(rel));
        }
    }
}
