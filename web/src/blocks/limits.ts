/**
 * How deeply blocks may nest: a card's own blocks are depth 1, and each
 * section, stack, column, list template or sheet adds one. Both validators
 * reject deeper cards (`max-depth`), and the renderer shows anything deeper
 * as unavailable, so a hostile card can't exhaust the stack. Real cards stay
 * far below it: the gallery's deepest is 3.
 * Mirrored as `MAX_DEPTH` in crates/blocks/src/lexicon.rs.
 */
export const MAX_DEPTH = 10
