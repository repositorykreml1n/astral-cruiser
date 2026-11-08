// Lives inside the flat draws' module, which keeps its tables private.
use super::Flat;

impl Flat {
    /// Swaps the section ids and block tables with `other`'s, the part of the flat
    /// draws that follows the arena pages. Each keeps its own pipelines, layout and
    /// origins buffer, so every page's bind group is built again on its next use.
    pub(crate) fn astral_swap_tables(&mut self, other: &mut Flat) {
        std::mem::swap(&mut self.pages, &mut other.pages);
        std::mem::swap(&mut self.free_ids, &mut other.free_ids);
        std::mem::swap(&mut self.next_id, &mut other.next_id);
        for flat in [&mut *self, &mut *other] {
            for page in flat.pages.iter_mut().flatten() {
                page.group = None;
            }
        }
    }
}
