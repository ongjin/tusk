# Week 2 — Manual Verification Checklist

Run after Task 11. Postgres docker must be up:

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm tauri dev
```

## Direct TCP

- [ ] `+ New connection` → Direct TCP tab → name `local`, host `127.0.0.1`,
      port `55432`, user/password `tusk`, database `tusk_test`, SSL `disable`.
      Save.
- [ ] Click plug icon → green dot, "Connected to local" toast.
- [ ] Schema tree expands to show `public` schema → tables (initially empty
      unless we seed) → no error.
- [ ] In the editor: `CREATE TABLE smoke (id int, name text);` then `Cmd+Enter`.
      Header: `0 rows · <X> ms`.
- [ ] `INSERT INTO smoke VALUES (1, 'a'), (2, 'b'), (3, NULL);` then
      `SELECT * FROM smoke;` — grid shows 3 rows, NULL italicised.

## SSH alias (your own ~/.ssh/config)

- [ ] `+ New connection` → SSH alias tab → list shows your hosts.
- [ ] Click an alias that maps to a Postgres bastion + ProxyJump (e.g. oci-db).
      Fill Postgres host (e.g. 127.0.0.1) / port (5432) / user / password /
      database. Save → connect.
- [ ] Schema tree loads.
- [ ] Run `SELECT version();` — succeeds.

## SSH manual

- [ ] `+ New connection` → SSH manual → SSH host/user/port/key path. Save and
      connect to a known reachable target.

## Editor / tabs

- [ ] `Cmd+T` opens a fresh tab. `Cmd+W` closes it. With one tab open,
      `Cmd+W` should reset to a fresh empty tab.
- [ ] `Cmd+Enter` runs the active tab's SQL.

## Result grid

- [ ] On a 50k-row table, `SELECT *` is auto-LIMITed to 1000.
- [ ] Setting auto-LIMIT to 0 disables auto-append (next run shows full set —
      or, given safety, run with `LIMIT 5000`).
- [ ] JSON cell expands on click.
- [ ] Sorting toggles arrows in the header.

## Error paths

- [ ] Wrong password → connect fails with toast (`TuskError(Connection)`).
- [ ] Bogus SSH alias → connect fails after ≤6s (`TuskError(Tunnel)`).
- [ ] Kill the tunnel from the OS (`pkill -f 'ssh -N -L'`) — within ~1s the
      sidebar dot turns grey, "Lost connection" toast.

## Theme + brand

- [ ] Toggle theme in light/dark — palette stays consistent (Tusk Amber
      visible on `+ New connection` button).
