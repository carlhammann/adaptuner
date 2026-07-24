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

    pub fn apply_to_old<X>(
        self,
        clone: impl Fn(&X) -> X,
        vec: &mut Vec<X>,
        selected: &mut Option<usize>,
    ) {
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
