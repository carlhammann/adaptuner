pub struct Subsequences {
    subseq_indices: i64,
    limit: i64,
    n: usize,
    subseq: Vec<usize>,
}

impl Subsequences {
    /// Generate all subsequences of the `n`-element list of length `k`. Subsequences that contain elements near
    /// the beginning of `seq` will be generated first.
    ///
    /// It must hold `62 >= n , k > 0`
    pub fn new(n: usize, k: usize) -> Self {
        Self {
            subseq_indices: (1 << k) - 1,
            limit: 1 << n,
            n,
            subseq: Vec::with_capacity(k),
        }
    }

    pub fn next(&mut self) -> Option<&[usize]> {
        if self.subseq_indices >= self.limit {
            return None {};
        }

        self.subseq.clear();
        for i in 0..self.n {
            if self.subseq_indices & (1 << i) != 0 {
                self.subseq.push(i);
            }
        }

        // Gosper's hack
        let c = self.subseq_indices & -self.subseq_indices;
        let r = self.subseq_indices + c;
        self.subseq_indices = (((r ^ self.subseq_indices) >> 2) / c) | r;

        Some(&self.subseq)
    }
}

#[cfg(test)]
mod test {
    use super::Subsequences;

    #[test]
    fn test_empty_subsequences() {
        let mut collected: Vec<Vec<usize>> = vec![];

        let mut subseqs = Subsequences::new(4, 5);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert!(collected.is_empty());
    }

    #[test]
    fn test_one_subsequences() {
        let mut collected: Vec<Vec<usize>> = vec![];

        let mut subseqs = Subsequences::new(4, 1);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert_eq!(collected, vec![vec![0], vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn test_two_subsequences() {
        let mut collected: Vec<Vec<usize>> = vec![];

        let mut subseqs = Subsequences::new(4, 2);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert_eq!(
            collected,
            vec![
                vec![0, 1],
                vec![0, 2],
                vec![1, 2],
                vec![0, 3],
                vec![1, 3],
                vec![2, 3]
            ]
        );
    }

    #[test]
    fn test_three_subsequences() {
        let mut collected: Vec<Vec<usize>> = vec![];

        let mut subseqs = Subsequences::new(4, 3);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert_eq!(
            collected,
            vec![vec![0, 1, 2], vec![0, 1, 3], vec![0, 2, 3], vec![1, 2, 3],]
        );
    }

    #[test]
    fn test_four_subsequences() {
        let mut collected: Vec<Vec<usize>> = vec![];

        let mut subseqs = Subsequences::new(4, 4);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert_eq!(collected, vec![vec![0, 1, 2, 3],]);
    }
}
