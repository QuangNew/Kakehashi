"""Snapshot/compare row identities and selected settings without reading credentials."""
import json
import os
import sqlite3
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
snapshot = root / '.cache/scope-verification/data-before.json'
if '--snapshot-file' in sys.argv:
    snapshot = Path(sys.argv[sys.argv.index('--snapshot-file') + 1]).resolve()
data = Path(os.environ['APPDATA']) / 'com.tvnjp.app'
config = json.loads((data / 'config.json').read_text(encoding='utf-8-sig'))
with sqlite3.connect((data / 'nexq.db').as_uri() + '?mode=ro', uri=True) as db:
    tables = [row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")]
    counts = {name: db.execute('SELECT COUNT(*) FROM "' + name.replace('"','""') + '"').fetchone()[0] for name in tables}
    meetings = [row[0] for row in db.execute('SELECT id FROM meetings ORDER BY id')]
selected = {key: config.get(key) for key in ['firstRunCompleted','recordingEnabled','meetingAudioConfig','llmProvider','llmModel']}
current = {'tables':counts,'meetingIds':meetings,'settings':selected}
snapshot.parent.mkdir(parents=True,exist_ok=True)
if '--snapshot' in sys.argv:
    snapshot.write_text(json.dumps(current,ensure_ascii=False,indent=2),encoding='utf-8')
    print(f'Snapshotted {len(tables)} tables and {len(meetings)} meeting identities; no credentials read')
else:
    before = json.loads(snapshot.read_text(encoding='utf-8'))
    assert current['tables'] == before['tables'], 'Database row counts changed'
    assert current['meetingIds'] == before['meetingIds'], 'Meeting identities changed'
    expected = before['settings']
    migrations = []
    if expected.get('meetingAudioConfig') and 'audio_mode' not in expected['meetingAudioConfig']:
        expected['meetingAudioConfig']['audio_mode'] = 'online'
        migrations.append('meetingAudioConfig.audio_mode=online')
    assert selected == expected, 'Selected user settings changed outside the approved migration'
    print(json.dumps({'preserved':True,'tables':len(tables),'meetings':len(meetings),'selectedSettings':len(selected),'expectedMigrations':migrations}))
