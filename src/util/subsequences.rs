pub struct Subsequences<'a, X> {
    subseq_indices: i64,
    limit: i64,
    seq: &'a [X],
    subseq: Vec<X>,
}

impl<'a, X: Clone> Subsequences<'a, X> {
    /// Generate all subsequences of `seq` of length `k`. Subsequences that contain elements near
    /// the beginning of `seq` will be generated first.
    ///
    /// It must hold `62 >= seq.len() , k > 0`
    pub fn new(seq: &'a [X], k: usize) -> Self {
        let n = seq.len();
        Self {
            subseq_indices: (1 << k) - 1,
            limit: 1 << n,
            seq,
            subseq: Vec::with_capacity(k),
        }
    }

    pub fn next(&mut self) -> Option<&[X]> {
        if self.subseq_indices >= self.limit {
            return None {};
        }

        self.subseq.clear();
        for (i, x) in self.seq.iter().enumerate() {
            if self.subseq_indices & (1 << i) != 0 {
                self.subseq.push(x.clone());
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
        let seq = ['a', 'b', 'c', 'd'];
        let mut collected: Vec<Vec<char>> = vec![];

        let mut subseqs = Subsequences::new(&seq, 5);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert!(collected.is_empty());
    }

    #[test]
    fn test_one_subsequences() {
        let seq = ['a', 'b', 'c', 'd'];
        let mut collected: Vec<Vec<char>> = vec![];

        let mut subseqs = Subsequences::new(&seq, 1);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert_eq!(collected, vec![vec!['a'], vec!['b'], vec!['c'], vec!['d']]);
    }

    #[test]
    fn test_two_subsequences() {
        let seq = ['a', 'b', 'c', 'd'];
        let mut collected: Vec<Vec<char>> = vec![];

        let mut subseqs = Subsequences::new(&seq, 2);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert_eq!(
            collected,
            vec![
                vec!['a', 'b'],
                vec!['a', 'c'],
                vec!['b', 'c'],
                vec!['a', 'd'],
                vec!['b', 'd'],
                vec!['c', 'd']
            ]
        );
    }

    #[test]
    fn test_three_subsequences() {
        let seq = ['a', 'b', 'c', 'd'];
        let mut collected: Vec<Vec<char>> = vec![];

        let mut subseqs = Subsequences::new(&seq, 3);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert_eq!(
            collected,
            vec![
                vec!['a', 'b', 'c'],
                vec!['a', 'b', 'd'],
                vec!['a', 'c', 'd'],
                vec!['b', 'c', 'd'],
            ]
        );
    }

    #[test]
    fn test_four_subsequences() {
        let seq = ['a', 'b', 'c', 'd'];
        let mut collected: Vec<Vec<char>> = vec![];

        let mut subseqs = Subsequences::new(&seq, 4);
        while let Some(l) = subseqs.next() {
            collected.push(l.into());
        }

        assert_eq!(collected, vec![vec!['a', 'b', 'c', 'd'],]);
    }
}
