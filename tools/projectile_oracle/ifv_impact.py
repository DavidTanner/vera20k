"""Original IFV launch through impact and bounded object retirement.
No Rust results or current-chain fixtures supply outputs.
"""
import importlib.util,json,struct,sys,hashlib,os
from pathlib import Path
from collections import deque
from unicorn import UC_HOOK_CODE,UC_HOOK_MEM_INVALID
from unicorn.x86_const import *
from capstone import Cs,CS_ARCH_X86,CS_MODE_32
from tools.native_oracle import NATIVE_SHA256,RET_MAGIC,run_checked,finish_vectors,provenance,initialize_empty_windows_seh,checked_is_bad_read_ptr_transport
from tools.spatial_oracle.building_body_rules import SP,dwords,INI
from tools.projectile_oracle.bridge_render_inputs import lexical,assets_root
# Isolate fixture adapters from other consumers of the shared launch harness.
spec=importlib.util.spec_from_file_location('_impact_launch',Path(__file__).with_name('ifv_launch.py'))
launch=importlib.util.module_from_spec(spec);spec.loader.exec_module(launch)
i32,xyz,vec=launch.i32,launch.xyz,launch.vec
base_prepare=launch.prepare
def initialize_effect_registries(m):
 """Original effect/world initializer regions shared by complete Anim witnesses."""
 u=m.u;m.invoke(0x561910,0)
 for start,end in ((0x4e7ae0,0x4e7b16),(0x7253d0,0x725406),(0x7252d0,0x725306),(0x725450,0x725486)):
  u.reg_write(UC_X86_REG_ESP,SP);run_checked(u,start,end)

def initialize_effect_world(m,cells,seed=31,*,map_size=(64,64),clear_terrain=True):
 """Supply the existing flat world's active/display state and seed natively."""
 u=m.u;u.mem_write(0xa8e9a0,b'\x01');u.mem_write(0x87f914,dwords(*map_size))
 for start,end in ((0x40b540,0x40b5ab),(0x725850,0x725886),(0x4e6d60,0x4e6d96)):
  u.reg_write(UC_X86_REG_ESP,SP);run_checked(u,start,end)
 from tools.rmg_oracle.gen_rng_vectors import seeded_struct
 u.mem_write(m.read32(0xa8b230)+0x218,seeded_struct(seed))
 if clear_terrain:
  for c in list(cells.values())+[0xabdc50]:
   u.mem_write(c+0x38,dwords(-1));u.mem_write(c+0x44,dwords(-1))

def retirement_transport(m,pc,events):
 """Only the existing Windows COM decrement/readability boundaries."""
 u=m.u;sp=u.reg_read(UC_X86_REG_ESP)
 if pc==0x46b007:
  ptr=m.read32(sp);value=(m.read32(ptr)-1)&0xffffffff
  u.mem_write(ptr,dwords(value));u.reg_write(UC_X86_REG_EAX,value)
  u.reg_write(UC_X86_REG_ESP,sp+4);u.reg_write(UC_X86_REG_EIP,0x46b00d)
  events.append(dict(event='OSInterlockedDecrement',field=hex(ptr),result=value))
  return True
 return checked_is_bad_read_ptr_transport(u,pc,sp,events)

def prepare():
 result=base_prepare();m=result[0];u=m.u;initialize_effect_registries(m)
 names=['XGRYSML1','XGRYSML2','EXPLOSML','XGRYMED1','XGRYMED2','EXPLOMED','EXPLOLRG','TWLT070']
 art,_=lexical((assets_root()/'ARTMD.INI').read_bytes(),set(names));m.make_ini(art);rows=[]
 for name in names:
  t=m.invoke(0x428b80,m.cstring(name));m.asset_loaded=[];admitted=m.invoke(0x427d00,t,(INI,))
  image=m.read32(t+0xa4)
  rows.append(dict(name=name,admitted=admitted&255,image_present=bool(image),frame_count=struct.unpack('<h',u.mem_read(image+6,2))[0] if image else None,end=i32(u,t+0x2c0),rate=i32(u,t+0x2b0),report=i32(u,t+0x2f8),scorch=bool(u.mem_read(t+0x36b,1)[0]),crater=bool(u.mem_read(t+0x36d,1)[0]),spawns_particle=i32(u,t+0x2cc),num_particles=i32(u,t+0x2d0),assets=m.asset_loaded))
 result[-1]['impact_anim_art']=rows
 return result
launch.prepare=prepare

def execute(bridge=True,origin=(2688,5248,1030),bridge_band=None,changes=()):
 m,b,cells,result=launch.launch(origin=origin,bridge=bridge);u=m.u
 initialize_effect_world(m,cells)
 if bridge_band is not None:
  for xy,c in cells.items():u.mem_write(c+0x140,dwords(0x100 if xy in bridge_band else 0))
  result['supplied']['bridge_band']=[list(xy) for xy in bridge_band]
 result['supplied']['between_frame_flag_changes']=[dict(frame=f,live=live) for f,live in changes]
 trace=deque(maxlen=32);events=[];pending={};raw_words=[];frame=0;area=False;fault=[];cs=Cs(CS_ARCH_X86,CS_MODE_32)
 addresses={0x5f4ec0:'NativeUnlimbo',0x4a9720:'DisplaySubmit',0x4690b0:'Detonate',0x489280:'AreaDamage',0x7258d0:'PointerExpired',0x5f65f0:'UnInit',0x466560:'BulletDtor',0x5f3b80:'ObjectDtor',0x725c70:'Drain',0x4a9770:'DisplayRemove',0x5f4d30:'Conceal',0x48a4f0:'SelectAnim',0x421ea0:'AnimCtor',0x424ce0:'AnimStart',0x424f00:'AnimMiddle',0x4255b0:'AnimDestroy',0x6b4a50:'SmudgeCtor',0x426590:'AnimScalarDtor',0x68bcb0:'NextNativeId',0x65c640:'Random1',0x65c660:'Random2',0x65c780:'RandomRaw',0x65c7e0:'RandomRanged',0x587180:'BridgeDriver',0x57baa0:'BridgeDriver2',0x57ccf0:'BridgeDriver3'}
 def invalid(uc,access,address,size,value,data):
  fault.append(dict(access=access,address=hex(address),size=size,pc=hex(u.reg_read(UC_X86_REG_EIP))));return False
 def obs(uc,a,n,d):
  nonlocal area
  trace.append(a)
  if a in pending:
   event=pending.pop(a);value=u.reg_read(UC_X86_REG_EAX);returned=dict(event=event['event']+'_return',frame=frame,pc=hex(a),eax=value)
   event['returned_eax']=value
   if event['event']=='AnimCtor':returned.update(native_id=i32(u,value+0x10),anim_type=m.string(m.read32(value+0xc8)+0x24),alive=u.mem_read(value+0x90,1)[0]);event['constructed_native_id']=i32(u,value+0x10)
   if event['event']=='NativeUnlimbo':returned.update(admitted=bool(value&255),retained=xyz(u,int(event['ecx'],16)+0x9c))
   if event['event']=='SelectAnim':returned['selected_name']=m.string(value+0x24) if value else None
   events.append(returned)
  if a in (0x65c84b,0x65c79d):raw_words.append(dict(frame=frame,pc=hex(a),word=u.reg_read(UC_X86_REG_ESI)))
  if a in addresses:
   sp=u.reg_read(UC_X86_REG_ESP);entry=dict(event=addresses[a],pc=hex(a),frame=frame,return_pc=hex(m.read32(sp)),ecx=hex(u.reg_read(UC_X86_REG_ECX)),args=[hex(m.read32(sp+4+j*4)) for j in range(4)])
   if a==0x5f4ec0:entry.update(position=xyz(u,m.read32(sp+4)))
   if a==0x421ea0:entry.update(anim_type=m.string(m.read32(sp+4)+0x24),position=xyz(u,m.read32(sp+8)))
   if a==0x48a4f0:entry.update(position=xyz(u,m.read32(sp+8)),retained_bullet=xyz(u,b+0x9c),land=i32(u,sp+4),damage=u.reg_read(UC_X86_REG_ECX),height_gate_unit=i32(u,0x89de70),level_height=i32(u,0x89e870))
   if a==0x489280:entry.update(position=xyz(u,u.reg_read(UC_X86_REG_ECX)),damage=u.reg_read(UC_X86_REG_EDX));area=True
   if a in (0x489280,0x48a4f0,0x421ea0,0x68bcb0,0x65c780,0x65c7e0,0x5f4ec0):pending[m.read32(sp)]=entry
   if a==0x65c7e0:entry.update(low=i32(u,sp+4),high=i32(u,sp+8))
   events.append(entry)
  retirement_transport(m,a,events)
  if a>=0x20000000 and a!=RET_MAGIC:raise AssertionError(('non-image-code',hex(a)))
 h=u.hook_add(UC_HOOK_CODE,obs);hi=u.hook_add(UC_HOOK_MEM_INVALID,invalid)
 frames=[]
 try:
  for frame in range(1,121):
   for change_frame,live in changes:
    if frame==change_frame:
     for xy in bridge_band:u.mem_write(cells[xy]+0x140,dwords(0x100 if live else 0))
   u.mem_write(0xa8ed84,dwords(frame));u.mem_write(SP,dwords(RET_MAGIC));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,b)
   run_checked(u,0x4666e0,RET_MAGIC,count=1000000)
   frames.append(dict(frame=frame,position=xyz(u,b+0x9c),velocity=vec(u,b+0xe8),alive=u.mem_read(b+0x90,1)[0]))
   if area:break
  result['before_drain']=dict(alive=u.mem_read(b+0x90,1)[0],queue_count=i32(u,0xb0f6a8),bullet_count=i32(u,0xa8ed50),anim_count=i32(u,0xa8e9b8))
  initialize_empty_windows_seh(u)
  m.invoke(0x725c70,0)
  result['after_drain']=dict(alive=u.mem_read(b+0x90,1)[0],queue_count=i32(u,0xb0f6a8),bullet_count=i32(u,0xa8ed50),anim_count=i32(u,0xa8e9b8))
  result['anim_frames']=[]
  for _ in range(100):
   if i32(u,0xa8e9b8)==0:break
   a=m.read32(m.read32(0xa8e9ac));frame+=1;u.mem_write(0xa8ed84,dwords(frame))
   m.invoke(0x423ac0,a)
   result['anim_frames'].append(dict(frame=frame,alive=u.mem_read(a+0x90,1)[0],anim_frame=i32(u,a+0xac),delay=i32(u,a+0x184),timer_start=i32(u,a+0xb4),timer_duration=i32(u,a+0xbc),queue_count=i32(u,0xb0f6a8)))
   if i32(u,0xb0f6a8):m.invoke(0x725c70,0)
  result['after_anim']=dict(queue_count=i32(u,0xb0f6a8),anim_count=i32(u,0xa8e9b8))
 except Exception as exc:
  result['failure']=dict(error=str(exc),fault=fault,registers={r:hex(u.reg_read(v)) for r,v in [('eax',UC_X86_REG_EAX),('ebx',UC_X86_REG_EBX),('ecx',UC_X86_REG_ECX),('edx',UC_X86_REG_EDX),('esi',UC_X86_REG_ESI),('edi',UC_X86_REG_EDI),('ebp',UC_X86_REG_EBP),('esp',UC_X86_REG_ESP)]},trace=[f'{a:08x}: '+ '; '.join(f'{x.mnemonic} {x.op_str}' for x in cs.disasm(bytes(u.mem_read(a,15)),a,count=1)) for a in trace])
 finally:u.hook_del(h);u.hook_del(hi)
 result.update(frames=frames,events=events,raw_rng_words=raw_words,final_scenario_native_id=i32(u,m.read32(0xa8b230)+0x214),rng_state_sha256=hashlib.sha256(bytes(u.mem_read(m.read32(0xa8b230)+0x218,0x3f4))).hexdigest());return result
def generate():
 band=[(x,20) for x in range(12,16)]
 specs=[dict(name='rise_live',bridge=True),dict(name='rise_clear',bridge=False),dict(name='band_live',bridge=False,origin=(2688,5248,1250),bridge_band=band),dict(name='band_clear16',bridge=False,origin=(2688,5248,1250),bridge_band=band,changes=((16,False),)),dict(name='band_clear16_repair24',bridge=False,origin=(2688,5248,1250),bridge_band=band,changes=((16,False),(24,True)))]
 rows=[]
 for case in specs:
  name=case.pop('name');row=execute(**case);row['name']=name
  assert 'failure' not in row,(name,row.get('failure'))
  assert row['after_drain']['queue_count']==0 and row['after_drain']['bullet_count']==0
  assert row['after_anim']['queue_count']==0 and row['after_anim']['anim_count']==0
  rows.append(row)
 return dict(native_sha256=NATIVE_SHA256,scope='Same five supplied launch/bridge-flag cases, physical DRAGON plus eight native HE AnimType ART readers, full original BulletAI/Detonate/AreaDamage with empty damage receivers and ScenarioDestroyableBridges=false, native impact Anim creation and Bullet retirement/drain, then single AnimAI until complete retirement. No damageable bridge topology, renderer, audio binding, Burst2 scheduler or whole Process/identity claim.',scenario_rng_seed=31,rows=rows)
def metadata():
 return provenance(scope='Five physical IFV impact controls: 125 original Bullet AI visits and 70 impact Anim AI visits through complete bounded retirement',assumptions=[
  'Original cell-spread table initializer561910, selected physical readers, FireAt launch, guided flight, full Detonate4690B0 and AreaDamage489280, impact SelectAnim/constructor/Start and full Bullet/Anim retirement execute. Earlier launch supplied boundaries remain: source lifecycle/FLH, Unlimbo/Display admission and inter-frame structural-bit changes.',
  'The shared launch boundary executes original Unlimbo admission-state writes, BulletType coordinate fixup and SetLocation. Correcting its former raw-coordinate copy clears InLimbo, so Conceal now executes DetachAll PointerExpired7258D0 and DisplayRemove4A9770 before drain. Every other retained payload field is unchanged by that boundary correction.',
  'Supplied GameActiveA8E9A0=1 admits original Anim Unlimbo/coordinate commit/Display submission. Flat level6 map cells, empty damage/air receivers, no tile/overlay identities and ScenarioDestroyableBridges=false. No damageable bridge topology; that is the separate ifv_bridge_impact fixture.',
  'Physical DRAGON and eight HE effect SHPs are read through native readers. Original partial pool construction gives Bullet29 then impact Anim30; no full startup/wholematch native-ID claim. Each control independently seeds Scenario RNG31.',
  'Host schedules pending drain after the Bullet tick, then one impact AnimAI per frame until its native retirement. All timer bodies execute; no whole Logic/Display scheduler claim. The LineTrail slot is null; ifv_trail_impact separately joins its admitted producer and detach.',
  'Shared selected reader setup is not full Rules Process chronology. No Rust expectation, native instruction patch, rendering or audio binding claim.',
 ],substitutions=[
  'Shared launch/input helpers supply lexical physical INI caches, archive bytes, allocation/delete/CRT/TLS and world admission. Verified imported InterlockedDecrement changes COM refcount only; original Release/destructors execute. IsBadReadPtr is false after mapped-memory validation, with an empty supplied FS exception chain.',
 ],entry_points={'bullet_ai':0x4666e0,'detonate':0x4690b0,'area_damage':0x489280,'select_anim':0x48a4f0,'anim_ctor':0x421ea0,'anim_ai':0x423ac0,'uninit':0x5f65f0,'expiry':0x7258d0,'conceal':0x5f4d30,'drain':0x725c70,'bullet_dtor':0x466560,'anim_dtor':0x426590})

if __name__=='__main__':
 finish_vectors(generate,Path(__file__).with_suffix('.json'),provenance=metadata)
