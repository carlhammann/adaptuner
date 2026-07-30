#[derive(Clone, Copy, PartialEq)]
pub enum ListAction {
    Delete(usize),
    Select(usize),
    Clone(usize),
    SwapWithPrev(usize),
    Deselect,
}

impl ListAction {
    pub fn apply_to_no_select<X>(self, vec: &mut Vec<X>, clone: impl Fn(&X) -> X) {
        match self {
            ListAction::Delete(i) => {
                vec.remove(i);
            }
            ListAction::Clone(i) => vec.push(clone(&vec[i])),
            ListAction::SwapWithPrev(i) => {
                vec.swap(i, i - 1);
            }
            ListAction::Select(_) => panic!("apply_to_no_select encountered ListAction::Select(_)"),
            ListAction::Deselect => panic!("apply_to_no_select encountered ListAction::Deselect"),
        }
    }

    pub fn apply_to<X>(self, vec: &mut Vec<X>, selected: &mut usize, clone: impl Fn(&X) -> X) {
        match self {
            ListAction::Delete(i) => {
                vec.remove(i);
                if *selected == 0 {
                    return;
                }
                if *selected >= i {
                    *selected -= 1;
                }
            }
            ListAction::Select(i) => {
                *selected = i;
            }
            ListAction::Clone(i) => vec.push(clone(&vec[i])),
            ListAction::SwapWithPrev(i) => {
                vec.swap(i, i - 1);
                if *selected == i {
                    *selected = i - 1;
                } else if *selected == i - 1 {
                    *selected = i;
                }
            }
            ListAction::Deselect => panic!("cannot deselect"),
        }
    }
}
