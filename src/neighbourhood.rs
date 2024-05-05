use crate::interval::StackCoeff;

// x x x
//   x x x
//     x x x
//
// x x x x
// x x x x
// x x x x
//
//     x x x x x
//   x x x x x
// x x x x x
//

///
/// - `width` must be in `1..=12`
/// - `offset` must be in `0..width`
/// and the element at
///
fn fivelimit_corridor(
    width: StackCoeff,
    offset: StackCoeff,
    index: StackCoeff,
) -> (StackCoeff, StackCoeff) {
    let (thirds, fifths) = fivelimit_corridor_no_offset(width, index + offset);
    (thirds, fifths - offset)
}

fn fivelimit_corridor_no_offset(width: StackCoeff, index: StackCoeff) -> (StackCoeff, StackCoeff) {
    let thirds = index.div_euclid(width);
    let fifths = (width - 4) * thirds + index.rem_euclid(width);
    (thirds, fifths)
}

pub fn fivelimit_neighbours(
    grid: &mut [(StackCoeff, StackCoeff); 12],
    width: StackCoeff,  // 1..=12
    index: StackCoeff,  // 0..=11
    offset: StackCoeff, // 0..=(width-1)
) {
    for i in (-index)..(12-index) {
        grid[(7 * i).rem_euclid(12) as usize] = fivelimit_corridor(width, offset, i);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_neighbours() {
        let mut grid = [(0, 0); 12];

        fivelimit_neighbours(&mut grid, 12, 0, 0);
        assert_eq!(
            grid,
            [
                (0, 0),
                (0, 7),
                (0, 2),
                (0, 9),
                (0, 4),
                (0, 11),
                (0, 6),
                (0, 1),
                (0, 8),
                (0, 3),
                (0, 10),
                (0, 5),
            ],
        );

        fivelimit_neighbours(&mut grid, 3, 0, 0);
        assert_eq!(
            grid,
            [
                (0, 0),
                (2, -1),
                (0, 2),
                (3, -3),
                (1, 0),
                (3, -1),
                (2, -2),
                (0, 1),
                (2, 0),
                (1, -1),
                (3, -2),
                (1, 1),
            ],
        );

        fivelimit_neighbours(&mut grid, 5, 0, 0);
        assert_eq!(
            grid,
            [
                (0, 0),
                (1, 3),
                (0, 2),
                (1, 5),
                (0, 4),
                (2, 3),
                (1, 2),
                (0, 1),
                (1, 4),
                (0, 3),
                (2, 2),
                (1, 1),
            ],
        );

        fivelimit_neighbours(&mut grid, 4, 0, 0);
        assert_eq!(
            grid,
            [
                (0, 0),
                (1, 3),
                (0, 2),
                (2, 1),
                (1, 0),
                (2, 3),
                (1, 2),
                (0, 1),
                (2, 0),
                (0, 3),
                (2, 2),
                (1, 1),
            ],
        );

        fivelimit_neighbours(&mut grid, 4, 1, 0);
        assert_eq!(
            grid,
            [
                (0, 0),
                (1, 3),
                (0, 2),
                (2, 1),
                (1, 0),
                (-1, 3),
                (1, 2),
                (0, 1),
                (2, 0),
                (0, 3),
                (2, 2),
                (1, 1),
            ],
        );

        fivelimit_neighbours(&mut grid, 4, 2, 0);
        assert_eq!(
            grid,
            [
                (0, 0),
                (1, 3),
                (0, 2),
                (2, 1),
                (1, 0),
                (-1, 3),
                (1, 2),
                (0, 1),
                (2, 0),
                (0, 3),
                (-1, 2),
                (1, 1),
            ],
        );

        fivelimit_neighbours(&mut grid, 4, 3, 0);
        assert_eq!(
            grid,
            [
                (0, 0),
                (1, 3),
                (0, 2),
                (-1, 1),
                (1, 0),
                (-1, 3),
                (1, 2),
                (0, 1),
                (2, 0),
                (0, 3),
                (-1, 2),
                (1, 1),
            ],
        );

        fivelimit_neighbours(&mut grid, 4, 4, 0);
        assert_eq!(
            grid,
            [
                (0, 0),
                (1, 3),
                (0, 2),
                (-1, 1),
                (1, 0),
                (-1, 3),
                (1, 2),
                (0, 1),
                (-1, 0),
                (0, 3),
                (-1, 2),
                (1, 1),
            ],
        );

        fivelimit_neighbours(&mut grid, 4, 0, 1);
        assert_eq!(
            grid,
            [
                (0, 0),
                (2, -1),
                (0, 2),
                (2, 1),
                (1, 0),
                (3, -1),
                (1, 2),
                (0, 1),
                (2, 0),
                (1, -1),
                (2, 2),
                (1, 1),
            ],
        );
        
        fivelimit_neighbours(&mut grid, 4, 0, 2);
        assert_eq!(
            grid,
            [
                (0, 0),
                (2, -1),
                (1, -2),
                (2, 1),
                (1, 0),
                (3, -1),
                (2, -2),
                (0, 1),
                (2, 0),
                (1, -1),
                (3, -2),
                (1, 1),
            ],
        );
        
        fivelimit_neighbours(&mut grid, 4, 0, 3);
        assert_eq!(
            grid,
            [
                (0, 0),
                (2, -1),
                (1, -2),
                (3, -3),
                (1, 0),
                (3, -1),
                (2, -2),
                (1, -3),
                (2, 0),
                (1, -1),
                (3, -2),
                (2, -3),
            ],
        );

    }
}
