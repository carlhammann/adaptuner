pub trait Config<A> {
    fn initialise(config: &Self) -> A;
}
