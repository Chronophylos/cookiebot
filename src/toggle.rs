pub trait Toggle {
    fn toggle(&mut self) -> Self;
}

impl Toggle for bool {
    fn toggle(&mut self) -> Self {
        let old = *self;
        *self = !*self;
        old
    }
}
