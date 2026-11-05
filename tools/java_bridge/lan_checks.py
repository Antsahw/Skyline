"""The Java mods' checks of tools/lan_e2e.py (`--java-mods DIR`: a folder of Fabric mod jars
both games run; the checks need the crittermod fixture: tools/java_bridge/fixture_mod.sh).

lan_e2e.py calls `register` once (its module) and `game_setup` for each game it starts."""
import shutil
from pathlib import Path

E = None  # tools/lan_e2e.py, once registered
CHECKS = ['javamobs', 'javahostile', 'javamount']


def register(lan_e2e):
    """Adds the checks to lan_e2e's CHECKS and CHECK_FNS."""
    global E
    E = lan_e2e
    E.CHECKS += CHECKS
    E.CHECK_FNS.update(javamobs=check_javamobs, javahostile=check_javahostile, javamount=check_javamount)


def game_setup(work, env):
    """With --java-mods: the jars in the game's mods-java/ (read beside its options), and the
    bridge's libraries (Fabric Loader, Fabric API) from this checkout's .local/bridge."""
    if not E.JAVA_MODS:
        return
    (work / 'mods-java').mkdir()
    for jar in Path(E.JAVA_MODS).glob('*.jar'):
        shutil.copy(jar, work / 'mods-java' / jar.name)
    env['HORIZON_ROOT'] = str(E.ROOT)


def java_entities(game, kind):
    """The Java mods' entities of `kind` the game shows (horizon-ctl `entities`)."""
    return [e for e in game.raw('entities', 16).get('entities', [])
            if e.get('kind') == 'java' and e.get('type') == kind]


def check_javamobs(host, guest, out):
    """A Java mod's mob on a LAN server (--java-mods with the crittermod fixture): the host summons
    critters; the guest's game draws them (the server's snapshot to each player) and the guest kills
    one with a sword (its attack packet, an id from the bridge's range, goes to the mods' server),
    which goes on both sides; one the host kills goes on the guest's side too."""
    if not E.JAVA_MODS:
        return 'skipped: no --java-mods'
    kind = 'horizon_critters:critter'
    for g in (host, guest):
        g('command', command='gamemode survival')
        g('command', command='clear @s')
        g('command', command='give @s minecraft:diamond_sword')
    E.face_each_other(host, guest)
    hx, hy, hz = E.pos_of(host)
    host('command', command=f'summon {kind} {hx + 1.5:.2f} {hy:.2f} {hz + 1.0:.2f} {{NoAI:1b}}')
    host('command', command=f'summon {kind} {hx + 1.5:.2f} {hy:.2f} {hz - 1.5:.2f} {{NoAI:1b}}')
    seen = {}
    seen['guest sees'] = len(guest.until(lambda: java_entities(guest, kind), lambda l: len(l) == 2, ticks=100))
    seen['host sees'] = len(java_entities(host, kind))
    guest.shot(out, 'javamobs-seen')
    # The guest's sword on the first: two full-charged hits (a critter has 8 health).
    target = min(java_entities(guest, kind), key=lambda e: abs(e['pos'][2] - (hz + 1.0)))
    tx, ty, tz = target['pos']
    for _ in range(2):
        guest.raw('look-at', f'{tx:.2f}', f'{ty + 0.45:.2f}', f'{tz:.2f}')
        guest.wait(25)
        guest.raw('mouse', 'left', 'click')
        guest.wait(5)
    seen['after guest kills'] = {g.name: len(g.until(lambda g=g: java_entities(g, kind), lambda l: len(l) <= 1, ticks=60))
                                 for g in (guest, host)}
    guest.shot(out, 'javamobs-killed')
    host('command', command=f'kill @e[type={kind}]')
    seen['after host kills'] = len(guest.until(lambda: java_entities(guest, kind), lambda l: not l, ticks=60))
    assert seen['guest sees'] == 2 and seen['host sees'] == 2, seen
    assert seen['after guest kills'] == {'guest': 1, 'host': 1}, seen
    assert seen['after host kills'] == 0, seen
    return seen


def check_javahostile(host, guest, out):
    """A Java mod's hostile mob on a LAN server (--java-mods with the crittermod fixture): a biter
    the host summons next to the guest (the host in creative, which it ignores) hunts the guest and
    bites; the guest's health drops (the mods' hurt of a mirrored player goes to Horizon, which
    hurts the guest as its own mobs do)."""
    if not E.JAVA_MODS:
        return 'skipped: no --java-mods'
    host('command', command='gamemode creative')
    host('command', command='difficulty normal')
    guest('command', command='gamemode survival')
    guest('command', command='effect give @s minecraft:instant_health 1 5 true')
    guest.wait(10)
    gx, gy, gz = E.pos_of(guest)
    before = guest('state')['player']['health']
    host('command', command=f'summon horizon_critters:biter {gx + 3:.2f} {gy:.2f} {gz:.2f}')
    after = guest.until(lambda: guest('state')['player']['health'], lambda h: h < before, ticks=200)
    guest.shot(out, 'javahostile')
    host('command', command='kill @e[type=horizon_critters:biter]')
    guest('command', command='effect give @s minecraft:instant_health 1 5 true')
    seen = {'before': before, 'after': after}
    assert after < before, seen
    return seen


def check_javamount(host, guest, out):
    """Riding a Java mod's mob on a LAN server (--java-mods with the crittermod fixture): the guest
    uses a mount the host summons beside it and sits on it (the mirror's startRiding; the server's
    snapshot gives the guest's game the seat), holds forward and the mount carries it (the mirror
    moves the mob by the guest's PlayerInput keys), then sneaks off it."""
    if not E.JAVA_MODS:
        return 'skipped: no --java-mods'
    kind = 'horizon_critters:mount'
    guest('command', command='gamemode creative')
    guest.wait(10)
    gx, gy, gz = E.pos_of(guest)
    host('command', command=f'summon {kind} {gx + 2.0:.2f} {gy:.2f} {gz:.2f}')
    mount = guest.until(lambda: java_entities(guest, kind), lambda l: len(l) == 1, ticks=100)
    seen = {'guest sees': len(mount)}
    mx, my, mz = mount[0]['pos']
    guest.raw('look-at', f'{mx:.2f}', f'{my + 0.5:.2f}', f'{mz:.2f}')
    guest.wait(5)
    guest.raw('mouse', 'right', 'click')
    on = guest.until(lambda: E.pos_of(guest), lambda p: p[1] - my > 0.3, ticks=60)
    seen['seated'] = round(on[1] - my, 2)
    # The host's game sees the guest sitting on the mount (the snapshot's riders).
    def guest_on_host():
        return [e for e in host.raw('entities', 16).get('entities', []) if 'player' in str(e.get('type'))]
    others = host.until(guest_on_host, lambda l: any(e.get('passenger') for e in l), ticks=40)
    seen['host sees it ride'] = any(e.get('passenger') and e.get('mount') == mount[0]['id'] for e in others)
    guest.raw('key', 'w', 'down')
    guest.wait(50)
    guest.raw('key', 'w', 'up')
    drove = E.pos_of(guest)
    seen['drove'] = round(((drove[0] - on[0]) ** 2 + (drove[2] - on[2]) ** 2) ** 0.5, 2)
    guest.raw('key', 'f5', 'tap')
    guest.wait(5)
    guest.shot(out, 'javamount')
    guest.raw('key', 'f5', 'tap')
    guest.raw('key', 'f5', 'tap')
    guest.raw('key', 'shift', 'down')
    guest.wait(20)
    guest.raw('key', 'shift', 'up')
    guest.wait(20)
    off = guest('state')['player']
    seen['off'] = {'on_ground': off['on_ground'], 'y over the mount': round(off['position'][1] - my, 2)}
    host('command', command=f'kill @e[type={kind}]')
    assert seen['guest sees'] == 1 and seen['seated'] > 0.3, seen
    assert seen['drove'] > 1.5, seen
    assert seen['host sees it ride'], seen
    assert seen['off']['y over the mount'] < 0.3, seen
    return seen
