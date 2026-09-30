"""Original FootApproach candidates with supplied raw occupation and geometry inputs.

Reuse the frozen full pursuit VM, readers, native hierarchy and MissionDispatch.
No rejection, candidate, range, cost, destination or RNG result is substituted.
"""
from pathlib import Path
import hashlib,json,struct,sys,tempfile
HERE=Path(__file__).resolve().parent
from . import pursuit as base
from .publication import finish_vectors
from tools.native_oracle import run_checked
from tools.spatial_oracle.building_body_rules import SP,dwords
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_EAX,UC_X86_REG_EBX,UC_X86_REG_EDI,UC_X86_REG_FPCW
# An unexpected fixture failure is diagnostic only, and belongs here.
base.HERE=Path(tempfile.gettempdir())/'vera20k-fv-cell-attack';base.HERE.mkdir(exist_ok=True)

class CandidateControls(base.Continuation):
 def reset(self):
  super().reset()
  for x,y,bits in getattr(self,'active_row',{}).get('ground_occupation',[]):
   assert self.m.read32(self.cells[x,y]+0x140)&0x100==0
   self.u.mem_write(self.cells[x,y]+0x124,bytes((bits,)))
 def observe_flow(self,pc,sp):
  super().observe_flow(pc,sp)
  if pc==0x4834A0:
   event=self.flow[-1];cell=int(event['this'],16)
   event.update(ground_occupation=self.u.mem_read(cell+0x124,1)[0],deck_occupation=self.u.mem_read(cell+0x128,1)[0])
 def control(self,row):
  self.active_row=row
  result=self.execute_case(row)
  return result

def geometry(q):
 """Two original interior numeric blocks; no search/admission claims."""
 q.active_row={};q.reset();q.phase='geometry'
 u=q.u;m=q.m;sp=SP-0x400;source=m.alloc(12)
 context=u.context_save()
 outputs=[]
 # Full quadrants, axes and fractional direction-boundary inputs. Source/target
 # coordinates are supplied to this numeric boundary, not authored Cell poses.
 poses=[([22400,13952],[22400,12414]),([22400,13952],[22400,15490]),
        ([22400,13952],[20862,13952]),([22400,13952],[23938,13952]),
        ([22400,13952],[20800,12352]),([22400,13952],[24000,12352]),
        ([22400,13952],[20800,15552]),([22400,13952],[24000,15552]),
        ([22400,13952],[22017,12352]),([22400,13952],[22783,12352]),
        ([22400,13952],[22401,12352]),([22400,13952],[22399,12352]),
        ([128,128],[-1500,-400]),([-128,-128],[1500,400]),
        ([22400,13952],[22400,13952])]
 for target,src in poses:
  u.context_restore(context);u.mem_write(source,dwords(*src,0));u.mem_write(sp+0x4C,dwords(*target,416))
  u.reg_write(UC_X86_REG_ESP,sp);u.reg_write(UC_X86_REG_EAX,source);u.reg_write(UC_X86_REG_EBX,q.src)
  run_checked(u,0x4D5C53,0x4D5CBC,count=100000,required_addresses=(0x4CAE30,0x7C5F00))
  word=struct.unpack('<H',u.mem_read(sp+0x1C,2))[0];direction=m.read32(sp+0xD0)
  rows=[]
  for radius,index in [(1228,i)for i in range(25)]+[(r,0)for r in (972,716,460,204,256,205)]:
   u.context_restore(context);u.mem_write(sp+0x24,dwords(0x8224DC+index*4));u.mem_write(sp+0x4C,dwords(*target,416));u.mem_write(sp+0xD0,dwords(direction));u.mem_write(sp+0x88,struct.pack('<d',radius))
   u.reg_write(UC_X86_REG_ESP,sp);u.reg_write(UC_X86_REG_EBX,q.src)
   run_checked(u,0x4D5FFE,0x4D6081,count=100000,required_addresses=(0x4CACB0,0x4CAD00,0x7C5F00))
   value=lambda x:struct.unpack('<i',dwords(x))[0]
   rows.append(dict(radius=radius,direction_index=index,offset=struct.unpack('<i',u.mem_read(0x8224DC+4*index,4))[0],unsnapped_xyz=[value(u.reg_read(UC_X86_REG_EAX)),struct.unpack('<i',u.mem_read(sp+0x38,4))[0],value(u.reg_read(UC_X86_REG_EDI))]))
  outputs.append(dict(target_xy=target,source_xy=src,facing_word=word,base_direction=direction,candidates=rows))
 return dict(fpcw=u.reg_read(UC_X86_REG_FPCW),poses=outputs,neighbor_offsets=[list(struct.unpack('<hh',u.mem_read(0x89F688+4*i,4)))for i in range(8)],scope='Original4D5C53..5CBC and4D5FFE..6081 only. Coordinates/radii are supplied; no admission, snap or actual map pose claimed for these numeric controls.')

def generate():
 q=CandidateControls(dict(name='prepare_healthy_candidates',stage='healthy'));q.plane();q.construct(True);q.pathfinder();q.graphs();q.checkpoint()
 rows=[dict(name='first_radial_clear'),
       dict(name='first_radial_ground_bit40',ground_occupation=[[87,49,0x40]]),
       dict(name='first_radial_and_sixth_neighbor_ground_bit40',ground_occupation=[[87,49,0x40],[89,50,0x40]]),
       dict(name='first_radial_and_later_neighbors_ground_bit40',ground_occupation=[[87,49,0x40],[89,50,0x40],[88,50,0x40]]),
       dict(name='first_radial_ground_bit01',ground_occupation=[[87,49,1]]),
       dict(name='first_radial_deeper_occupation',ground_occupation=[[x,y,0x40]for y in (49,50)for x in range(85,91)])]
 for row in rows:row.update(stage='healthy',source_y_delta=-2)
 try:
  results=[q.control(row)for row in rows]
  numeric=geometry(q)
  result=dict(schema=1,native_sha256=base.NATIVE_SHA256,original_constants=base.original_constants(),cases=results,numeric=numeric)
  assert base.proof.sha(bytes(q.u.mem_read(base.proof.TEXT_START,base.proof.TEXT_SIZE)))==q.prefix['text_sha256']
  return result
 finally:q.close()

def metadata():
 result=base.metadata();result['scope']=__doc__;result['pursuit_harness_sha256']=result['harness_sha256'];result['harness_sha256']=base.proof.sha(Path(__file__).read_bytes());result['assumptions'] += [
  'Additional complete MissionDispatch controls begin with supplied retained Cell+124 raw ground-occupation bytes. Native4834A0 reads them and determines rejection. No owner/actor/mark producer of these inputs is claimed; underlying frozen Terrain/class/zone data is unchanged. These input controls are not manufactured callback returns.',
  'Numeric-only controls enter original4D5C53 and4D5FFE with declared stack/source inputs in the same native-initialized VM, ambientFPCW0E3F. Native facing conversion and Sin/Cos/ftol execute; no numeric expected result is hand-computed. Negative/zero-delta poses and shorter radii are boundary inputs, not stock map states.'
 ];return result

if __name__=='__main__':finish_vectors(generate,HERE/'candidates.json.gz',provenance=metadata)
