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
            ListAction::Select(_) => {}
            ListAction::Deselect => {}
        }
    }

    pub fn apply_to<X>(
        self,
        vec: &mut Vec<X>,
        selected: usize,
        clone: impl Fn(&X) -> X,
        mut replace_selected: impl FnMut(usize),
    ) {
        match self {
            ListAction::Delete(i) => {
                vec.remove(i);
                if selected == 0 {
                    return;
                }
                if selected >= i {
                    replace_selected(selected - 1);
                }
            }
            ListAction::Select(i) => replace_selected(i),
            ListAction::Clone(i) => vec.push(clone(&vec[i])),
            ListAction::SwapWithPrev(i) => {
                vec.swap(i, i - 1);
                if selected == i {
                    replace_selected(i - 1);
                } else if selected == i - 1 {
                    replace_selected(i);
                }
            }
            ListAction::Deselect => panic!("cannot deselect"),
        }
    }
}
