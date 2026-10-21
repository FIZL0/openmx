"""S74: shared lookups for the put-back bone captures. Research, not product."""

GAME_MANAGER = 0x004E1E98
FRAME_COUNTER = 0x00479D04
FRAME_DT = 0x00479CD4
CONTROLS = 0x00678F08


def chain(p, addr, *offs):
    for off in offs:
        addr = p.read32(addr)
        if not addr or addr < 0x100000 or addr >= 0x2000000:
            return 0
        addr += off
    return addr


def actors(p):
    node = p.read32(chain(p, GAME_MANAGER, 0x34, 0x30 + 4))
    for _ in range(64):
        if not node:
            break
        item = p.read32(node)
        if item:
            yield item, p.read32(item)
        nxt = p.read32(node + 8)
        if nxt == node:
            break
        node = nxt


def human_bike(p):
    for item, kind in actors(p):
        if kind == 2:
            player = p.read32(item + 0x20)
            if player and p.read32(player + 0x34) == 1:
                return item
    return 0


def rider_body(p, bike):
    """The rider body actor (the type-4 record less 0x400) whose request block is this bike's."""
    seen = []
    for item, kind in actors(p):
        if kind == 4:
            body = item - 0x400
            req = p.read32(body + 0x488)
            seen.append((item, req))
            if req == bike + 0xb80:
                return body
    print('type-4 records and their requests:', [(f'{i:08x}', f'{r:08x}') for i, r in seen])
    return 0
