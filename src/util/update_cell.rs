use std::cell::{Ref, RefCell};

/// A [RefCell]-like type that allows an update function. This cell can either contain someting or
/// nothing. If you apply an action that expects the cell to be non-empty, it will panic.
pub struct UpdateCell<X>(RefCell<Option<X>>);

impl<X> UpdateCell<X> {
    pub fn new(value: X) -> Self {
        Self(RefCell::new(Some(value)))
    }

    pub fn set(&self, value: X) {
        let Self(cell) = self;
        cell.replace(Some(value));
    }

    pub fn take(&self) -> X {
        let Self(cell) = self;
        cell.replace(None {})
            .unwrap_or_else(|| panic!("tried to take from empty UpdateCell"))
    }

    pub fn update(&self, f: impl FnOnce(X) -> X) {
        let Self(cell) = self;
        let old = cell
            .replace(None {})
            .unwrap_or_else(|| panic!("tried to update empty UpdateCell"));
        cell.replace(Some(f(old)));
    }

    pub fn borrow(&self) -> Ref<'_, X> {
        let Self(cell) = self;
        Ref::map(cell.borrow(), |x| {
            x.as_ref()
                .unwrap_or_else(|| panic!("tried to borrow empty UpdateCell"))
        })
    }
}
