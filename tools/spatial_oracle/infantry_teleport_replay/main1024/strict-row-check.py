from pathlib import Path
import argparse,gzip,json
parser=argparse.ArgumentParser(description='Compare complete replay rows, excluding only the hash feed.')
parser.add_argument('--directory',type=Path,default=Path(__file__).resolve().parent)
parser.add_argument('--replay',default='global')
parser.add_argument('--rows',type=int,default=601)
args=parser.parse_args()
root=args.directory
rows={mode:[json.loads(line) for line in gzip.decompress((root/f'{mode}-{args.replay}.jsonl.gz').read_bytes()).decode().splitlines()] for mode in ('control','current')}
assert len(rows['control'])==len(rows['current'])==args.rows
expected=json.loads((root/'receipt.json').read_text())['final_hashes']
for mode in rows:
 assert rows[mode][-1]['tick_result']['state_hash']==expected[mode]
 for row in rows[mode]:
  if row['tick_result'] is not None:row['tick_result'].pop('state_hash')
assert rows['control']==rows['current']
print(f'PASS {args.rows} complete {args.replay} rows differ only in tick_result.state_hash')
