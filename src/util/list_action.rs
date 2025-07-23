#[derive(Clone)]
pub enum ListAction<X> {
    Delete(usize),
    SwapWithPrev(usize),
    Select(usize),
    Deselect,
    Add(X),
}

impl<X> ListAction<X> {
    pub fn map<Y>(self, f: impl Fn(X) -> Y) -> ListAction<Y> {
        match self {
            ListAction::Delete(i) => ListAction::Delete(i),
            ListAction::SwapWithPrev(i) => ListAction::SwapWithPrev(i),
            ListAction::Select(i) => ListAction::Select(i),
            ListAction::Deselect => ListAction::Deselect,
            ListAction::Add(x) => ListAction::Add(f(x)),
        }
    }

    pub fn apply_to(self, vec: &mut Vec<X>, selected: &mut Option<usize>) {
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
            ListAction::Add(x) => {
                vec.push(x);
                if let Some(j) = selected {
                    *j = vec.len() - 1;
                }
            }
        }
    }
}
