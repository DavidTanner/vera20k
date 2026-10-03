from pathlib import Path
import gzip,json
root=Path(__file__).resolve().parent
rows={mode:[json.loads(line) for line in gzip.decompress((root/f'{mode}-global.jsonl.gz').read_bytes()).decode().splitlines()] for mode in ('control','current')}
assert len(rows['control'])==len(rows['current'])==601
expected=json.loads((root/'receipt.json').read_text())['final_hashes']
for mode in rows:
 assert rows[mode][-1]['tick_result']['state_hash']==expected[mode]
 for row in rows[mode]:
  if row['tick_result'] is not None:row['tick_result'].pop('state_hash')
assert rows['control']==rows['current']
print('PASS 601 complete rows differ only in tick_result.state_hash')
