# Schedule SimpleFIN sync

`nigel sync` is safe to run unattended: it imports only transactions it has not seen, snapshots first whenever it has rows to write, and an account with nothing new writes no import. A systemd user timer runs it on a schedule.

Set SimpleFIN up and map accounts by hand first (`nigel sync setup`, `nigel sync accounts --map …`). An encrypted database also needs the unattended password described under "Unattended password" in [commands.md](../commands.md).

`~/.config/systemd/user/nigel-sync.service`:

```ini
[Unit]
Description=Sync transactions from SimpleFIN into Nigel
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.cargo/bin/nigel sync
# For encrypted books:
# Environment=NIGEL_DB_PASSWORD_FILE=%h/.config/nigel/db-password
```

`~/.config/systemd/user/nigel-sync.timer`:

```ini
[Unit]
Description=Sync Nigel from SimpleFIN every morning

[Timer]
OnCalendar=*-*-* 06:30
RandomizedDelaySec=15m
Persistent=true

[Install]
WantedBy=timers.target
```

Enable it, and keep it running while you are logged out:

```bash
systemctl --user daemon-reload
systemctl --user enable --now nigel-sync.timer
loginctl enable-linger "$USER"
```

Check the last run with `journalctl --user -u nigel-sync.service`, and the next with `systemctl --user list-timers nigel-sync.timer`. SimpleFIN bridges refresh from the banks about once a day, so more than a daily run gains nothing. A run that imports anything leaves a `pre-sync-*.db` snapshot under the data directory's `snapshots/`.
