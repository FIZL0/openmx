"""C31: write research/c31/c31.toml, the watch spec for an sx01 race's score cards.

The activity is *(*(*activity_manager + 0x58)); the score manager is embedded at its +0x50, its first
card at activity +0x84 (= manager +0x34), each next card at card +0x1c8. Card k is that chain with k
extra `-> 0x1c8` hops. Every card gets its named words (the ones the lap line, S13c's `LapCard`, and
the race handler, score_race_tick, read or write) and its whole first 0x1d0 bytes raw, so a word not
named here can still be read off the capture later.

The original re-sorts that list by place every few ticks, so a list position is not a rider, and a
re-sort between the recorder's pointer reads tears the row. Given BASE and STRIDE, the cards are read
at fixed addresses instead (card k at BASE + k * STRIDE, which is where card id k sat in sx01-ai4),
one column group per rider; the list order is still in each raw block (manager +0x34, card +0x1c8).

usage: python3 research/c31/spec.py [CARDS [BASE STRIDE]] > research/c31/c31.toml
"""
import sys

cards = int(sys.argv[1]) if len(sys.argv) > 1 else 4
base, stride = (int(sys.argv[2], 0), int(sys.argv[3], 0)) if len(sys.argv) > 3 else (None, None)
ACT = '@activity_manager -> 0x58 -> 0x0'

# (name, offset, type): the card's words, as the lap line and the race handler use them.
CARD = [
    ('id', 0x00, 's32'),
    ('flags', 0x08, 'u64'),
    ('to_line_kept', 0x10, 'f32'), ('to_line', 0x14, 'f32'), ('path_to', 0x18, 'f32'),
    ('progress', 0x1c, 'f32'), ('travelled', 0x20, 'f32'),
    ('laps_length', 0x24, 'f32'), ('lap_length', 0x28, 'f32'),
    ('place', 0x2c, 's32'), ('laps', 0x30, 's32'), ('runs', 0x34, 's32'),
    ('on_track', 0x40, 's32'),
    ('lap_time', 0x48, 'f32'), ('lap_start', 0x4c, 'f32'), ('best_lap', 0x50, 'f32'), ('best_lap_no', 0x54, 's32'),
    ('now', 0x60, 'f32x4'), ('last', 0x70, 'f32x4'),
    ('spot_path', 0x88, 'u32'), ('spot_piece', 0x8c, 'u32'), ('spot_along', 0x90, 'f32'),
    ('kept_path', 0x94, 'u32'), ('kept_piece', 0x98, 'u32'), ('kept_along', 0x9c, 'f32'),
    ('race_time', 0xb4, 'f32'), ('lap_clock', 0xb8, 'f32'), ('behind', 0xbc, 'f32'),
    ('off_clock', 0xc0, 'f32'), ('w_c4', 0xc4, 'f32'),
    ('lines', 0xe4, 'u32'), ('record_word', 0x108, 'u32'), ('records', 0x1bc, 'u32'),
    ('raw', 0x00, 'bytes:464'),
]

out = []
w = out.append
w('# c31.toml -- written by research/c31/spec.py; do not edit by hand.')
w('# C31: an sx01 single race, every score card per tick (research/c31/README.md).')
w('')
w('[watch]')
w('name        = "c31-sx01-laps"')
w('description = "sx01 single race, %d AI-ridden riders: the score manager and every card per tick"' % cards)
w('every       = 1')
w('frames      = 12000')
w('start       = "in_game == 1"')
w('')
w('[setup]')
w('warmup = 0')
w('')
for name, addr, typ in [
    ('frame_counter', '@frame_counter', 'u32'),
    ('dt', '@frame_dt', 'f32'),
    ('rng_state', '@rand_state', 'u64'),
    ('in_game', '0x004e2650', 'u32'),
]:
    w('[[field]]\nname = "%s"\naddr = "%s"\ntype = "%s"\n' % (name, addr, typ))
# The score manager: its flags, kind, laps, countdown and the words score_race_tick reads.
for name, off, typ in [
    ('mgr_flags', 0x5c, 'u32'), ('mgr_kind', 0x60, 's32'), ('mgr_handler', 0x64, 's32'), ('mgr_laps', 0x94, 's32'),
    ('mgr_b8', 0xb8, 'f32'), ('mgr_count_len', 0xbc, 'f32'), ('mgr_count_elapsed', 0xc4, 'f32'),
    ('mgr_raw', 0x00, 'bytes:272'),
]:
    w('[[field]]\nname = "%s"\naddr = "%s -> 0x%x"\ntype = "%s"\n' % (name, ACT, 0x50 + off, typ))
for k in range(cards):
    w('[[array]]')
    w('name  = "card%d_"' % k)
    if base is None:
        w('addr  = "%s -> 0x84%s"' % (ACT, ' -> 0x1c8' * k))
        w('deref = true')
    else:
        w('addr  = "0x%x"' % (base + k * stride))
    w('count = 1')
    w('')
    for name, off, typ in CARD:
        w('[[array.field]]\nname = "%s"\noff  = 0x%x\ntype = "%s"\n' % (name, off, typ))
print('\n'.join(out))
