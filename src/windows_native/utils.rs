pub trait PeakIterExt<T: Iterator> {
    fn peaking(self) -> PeakingIter<T>;
}

impl<T: Iterator> PeakIterExt<T> for T {
    fn peaking(mut self) -> PeakingIter<T> {
        PeakingIter {
            next: self.next(),
            inner: self,
        }
    }
}

pub struct PeakingIter<T: Iterator> {
    inner: T,
    next: Option<T::Item>
}

impl<T: Copy, I: Iterator<Item=T>> Iterator for PeakingIter<I> {
    type Item = (I::Item, Option<I::Item>);

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next.take();
        self.next = self.inner.next();
        current
            .map(|v| (v, self.next))
    }
}