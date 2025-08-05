#[derive(Clone, Copy, PartialEq)]
pub enum ListAction {
    Delete(usize),
    SwapWithPrev(usize),
    Select(usize),
    Deselect,
    Clone(usize),
}

impl ListAction {

    pub fn apply_to<X>(self, clone: impl Fn(&X) -> X, vec: &mut Vec<X>, selected: &mut Option<usize>) {
        match self {
            ListAction::Delete(i) => {
                vec.remove(i);
                if let Some(j) = selected {
                    if *j == 0 {
                        return;
                    }
                    if *j >= i {
                        *j -= 1;
                    }
                }
            }
            ListAction::SwapWithPrev(i) => {
                vec.swap(i, i - 1);
                if let Some(j) = selected {
                    if *j == i {
                        *j = i - 1;
                    } else if *j == i - 1 {
                        *j = i;
                    }
                }
            }
            ListAction::Select(i) => {
                *selected = Some(i);
            }
            ListAction::Deselect => {
                *selected = None {};
            }
            ListAction::Clone(i) => {
                vec.push(clone(&vec[i]));
                if let Some(j) = selected {
                    *j = vec.len() - 1;
                }
            }
        }
    }
}
