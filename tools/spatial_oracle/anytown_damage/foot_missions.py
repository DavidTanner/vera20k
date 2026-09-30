"""Selected original Foot missions, threat wrapper, idle and downstream receipts using Anytown Mission.

Invoke through mission --foot-missions. No scanner, approach, RNG or map decision
is reimplemented here. Explicit prior-state controls and the scanner-return seam
are distinguished from the original full scanner rows; see foot_missions.md.
"""
import os, sys, json, hashlib, struct, time
from pathlib import Path
from collections import deque

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]

from tools.spatial_oracle.anytown_damage import mission as native_owner
from tools.native_oracle import RET_MAGIC, NATIVE_SHA256, run_checked, finish_vectors, provenance, file_span, image_bytes
from tools.spatial_oracle.building_body_rules import INI, RULES, SP, dwords
from tools.projectile_oracle.bridge_render_inputs import lexical
from tools.spatial_oracle.bridge_target_composed import GENERAL_KEYS
from tools.spatial_oracle.locomotor_at_coord import FAMILIES
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import *

class FootMissions(native_owner.Mission):
 def observe(self,u,a,n,d):
  if a==0x517B74:
   sp=u.reg_read(UC_X86_REG_ESP)
   clsid,outer,context,iid,ppv=struct.unpack('<5I',u.mem_read(sp,20))
   assert self.m.read32(self.e1_type+0x34C)==self.m.read32(clsid)
   assert (outer,context)==(0,7)
   u.mem_write(sp,dwords(a+6,0,outer,iid,ppv));u.reg_write(UC_X86_REG_EIP,0x6C4790)
   self.events.append(dict(kind='COM_Walk_original_factory',factory='0x006C4790',clsid=bytes(u.mem_read(clsid,16)).hex(),iid=bytes(u.mem_read(iid,16)).hex()))
   return
  if a==0x517B83:
   sp=u.reg_read(UC_X86_REG_ESP)
   self.events.append(dict(kind='OS_OleRun',interface=hex(self.m.read32(sp))))
   u.reg_write(UC_X86_REG_EAX,0);u.reg_write(UC_X86_REG_ESP,sp+4);u.reg_write(UC_X86_REG_EIP,a+6)
   return
  super().observe(u,a,n,d)

 def initialize_companion(self):
  self.setup();m,u=self.m,self.u
  self.phase='setup'
  # Existing owner's registry convention, now for the original InfantryType.
  u.mem_write(0xA8E348,dwords(0x7EB6D4,m.alloc(4096),1024,1,0,10))
  u.mem_write(0xA83DE8,dwords(0x7EB6D4,m.alloc(4096),1024,1,0,10))
  self.e1_type=m.alloc(0x1900)
  m.invoke(0x5236A0,self.e1_type,(m.cstring('E1'),))
  art,_=lexical((Path(os.environ['VERA20K_SHRAPNEL_INPUTS'])/'ARTMD.INI').read_bytes(),{'E1','GI','GISequence','MTNK','GTNK'})
  m.make_ini(art)
  self.mission_inputs=[];self.e1_weapon_read_attempts=[]
  for name,path in native_owner.base.layers():
   if not path.exists():
    assert name=='LANGRULE.INI';self.mission_inputs.append(dict(file=name,absent=True));continue
   raw=path.read_bytes();selected,lines=lexical(raw,{'Rescue','Area Guard','Guard','Attack','E1','M60','M60E','Invisible','SA','General','ElevationModel'})
   general={key:value for key,value in selected.get('General',{}).items() if key in GENERAL_KEYS}
   m.rules_cache({'General':general} if general else {});m.invoke(0x668BF0,self.rules,(RULES,))
   m.rules_cache(selected)
   elevation=m.invoke(0x66D150,self.rules,(RULES,))
   u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ESI,RULES)
   run_checked(u,0x679C92,0x679CAF,required_addresses=[0x5B3760])
   e1=m.invoke(0x5240A0,self.e1_type,(RULES,))
   weapon=m.read32(self.e1_type+0x898)
   if 'M60' in selected:
    weapon_al=m.invoke(0x772080,weapon,(RULES,))&255
    projectile=m.read32(weapon+0xA0);warhead=m.read32(weapon+0xAC)
    projectile_al=m.invoke(0x46BEE0,projectile,(RULES,))&255;warhead_al=m.invoke(0x75D3A0,warhead,(RULES,))&255
    m.invoke(0x7729F0,weapon)
    self.e1_weapon_read_attempts.append(dict(file=name,weapon=dict(entry='0x00772080',name=m.string(weapon+0x24),returned_al=weapon_al),
     projectile=dict(entry='0x0046BEE0',name=m.string(projectile+0x24),returned_al=projectile_al),
     warhead=dict(entry='0x0075D3A0',name=m.string(warhead+0x24),returned_al=warhead_al),postpass='0x007729F0'))
   self.mission_inputs.append(dict(file=name,sha256=hashlib.sha256(raw).hexdigest(),inputs=selected,lines=lines,
    mission={str(k):bytes(u.mem_read(0xA8E3A8+k*32,32)).hex() for k in (5,11,21)},
    e1_reader_al=e1&255,e1_image=m.string(self.e1_type+0x1F8),e1_range=native_owner.base.i32(u,weapon+0xB4),
    e1_locomotor=bytes(u.mem_read(self.e1_type+0x34C,16)).hex(),elevation_reader_al=elevation&255,elevation_increment=m.read32(self.rules+0x1838),
    elevation_bonus_bits=bytes(u.mem_read(self.rules+0x1840,8)).hex(),elevation_bonus_cap_bits=bytes(u.mem_read(self.rules+0x1848,8)).hex()))
  self.e1_weapon=m.read32(self.e1_type+0x898)
  self.e1=m.alloc(0x1000);m.invoke(0x517A50,self.e1,(self.e1_type,0))
  self.e1_ctor=dict(actor=hex(self.e1),vtable=hex(m.read32(self.e1)),loco=hex(m.read32(self.e1+0x674)),
   archive=hex(m.read32(self.e1+0x218)),target=hex(m.read32(self.e1+0x2B4)),health=native_owner.base.i32(u,self.e1+0x6C),
   flags=m.read32(self.e1+0x14),limbo=u.mem_read(self.e1+0x81,1)[0],doing=native_owner.base.i32(u,self.e1+0x6C4))
  initial_ctor=self.initial_action_snap(self.e1)
  u.mem_write(self.e1+0x21C,dwords(self.house));u.mem_write(self.e1+0x14C,dwords(self.house))
  # Same supplied House count-vector contract as the existing Unit owner;
  # original Infantry Unlimbo indexes its distinct live-count vector+5578.
  for off in (0x5528,0x5578):
   u.mem_write(self.house+off+4,dwords(m.alloc(4096),1024));u.mem_write(self.house+off+0x10,dwords(0))
  self.phase='placement'
  self.e1_unlimbo=[]
  for xy in ((87,49),(88,50),(86,50),(88,49)):
   m.invoke(0x486840,self.resident.ptrs[xy],(self.coord,))
   value=m.invoke(0x51DFF0,self.e1,(self.coord,0x80))
   self.e1_unlimbo.append(dict(cell=list(xy),al=value&255))
   if value&255:break
  assert self.e1_unlimbo[-1]['al'],self.e1_unlimbo
  self.initial_action_receipt=self.probe_initial_action(initial_ctor)
  self.e1_ready=m.invoke(0x51D6F0,self.e1,(0,0,0))
  self.e1_placement=dict(doing=native_owner.base.i32(u,self.e1+0x6C4),xyz=native_owner.base.xyz(u,self.e1+0x9C),on_bridge=u.mem_read(self.e1+0x8C,1)[0])
  self.baselines={p:bytes(u.mem_read(p,0x1000)) for p in (self.src,self.e1)}
  self.loco_baselines={m.read32(p+0x674)-4:bytes(u.mem_read(m.read32(p+0x674)-4,0x100)) for p in (self.src,self.e1)}
  self.phase='setup'
  self.initializers=[]
  for table in (0x8141D8,0x815040):
   for address in struct.unpack('<14I',u.mem_read(table,56)):
    m.invoke(address,0);self.initializers.append(hex(address))
  self.enemy=m.alloc(0x16000)
  u.mem_write(self.enemy+0x30,dwords(1,self.country));u.mem_write(self.enemy+0x188,struct.pack('<d',1.0));u.mem_write(self.enemy+0x1A8,struct.pack('<d',1.0));u.mem_write(self.enemy+0x1EC,b'\1')
  for house in (self.house,self.enemy):
   u.mem_write(house+0x5600,dwords(-1))
   for reg,value in ((UC_X86_REG_EBP,house),(UC_X86_REG_ESI,self.country),(UC_X86_REG_EBX,0)):u.reg_write(reg,value)
   run_checked(u,0x4F643B,0x4F6455)
  self.victim=m.alloc(0x1000);m.invoke(0x7353C0,self.victim,(self.typ,0))
  self.candidate=m.alloc(0x1000);m.invoke(0x7353C0,self.candidate,(self.typ,0))
  for p,house,xyz in ((self.victim,self.house,(23040,12928,416)),(self.candidate,self.enemy,(23296,12928,416))):
   u.mem_write(p+0x21C,dwords(house));u.mem_write(p+0x9C,dwords(*xyz))
  for off in (0x5514,0x5564):
   u.mem_write(self.enemy+off+4,dwords(m.alloc(4096),1024));u.mem_write(self.enemy+off+0x10,dwords(0))
  self.candidate_unlimbo=[];self.phase='placement'
  for xy in ((88,50),(86,50),(88,49),(86,49)):
   m.invoke(0x486840,self.resident.ptrs[xy],(self.coord,))
   value=m.invoke(0x737BA0,self.candidate,(self.coord,0x80))
   self.candidate_unlimbo.append(dict(cell=list(xy),al=value&255))
   if value&255:break
  assert self.candidate_unlimbo[-1]['al'],self.candidate_unlimbo
  self.candidate_placement=dict(xyz=native_owner.base.xyz(u,self.candidate+0x9C),display_layer=m.read32(self.candidate+0x94),flags=m.read32(self.candidate+0x14),limbo=u.mem_read(self.candidate+0x81,1)[0])
  self.phase='placement';m.invoke(0x486840,self.resident.ptrs[86,50],(self.coord,))
  self.victim_unlimbo=m.invoke(0x737BA0,self.victim,(self.coord,0x80))
  assert self.victim_unlimbo&255
  self.victim_placement=dict(xyz=native_owner.base.xyz(u,self.victim+0x9C),display_layer=m.read32(self.victim+0x94),flags=m.read32(self.victim+0x14),limbo=u.mem_read(self.victim+0x81,1)[0])
  self.prototype_baselines={p:bytes(u.mem_read(p,0x1000)) for p in (self.victim,self.candidate)}
  self.cell_baselines={p:bytes(u.mem_read(p,0x200)) for p in self.resident.ptrs.values()}
  self.entry_getcoords={m.read32(m.read32(p)+0x48) for p in (self.src,self.e1,self.victim,self.candidate,*self.resident.ptrs.values())}
  self.vtables={hex(v):bytes(u.mem_read(v,0x600)).hex() for v in {m.read32(p) for p in (self.src,self.e1,self.victim,self.candidate,*self.resident.ptrs.values())}}
  self.phase='logic';self.events.clear();self.pending.clear()

 def snap(self,p):
  m,u=self.m,self.u
  return dict(mission=native_owner.base.i32(u,p+0xAC),queued=native_owner.base.i32(u,p+0xB4),status=native_owner.base.i32(u,p+0xBC),
   archive=hex(m.read32(p+0x218)),target=hex(m.read32(p+0x2B4)),nav=hex(m.read32(p+0x5A4)),
   dispatch=[native_owner.base.i32(u,p+0xC8),native_owner.base.i32(u,p+0xD0)],
   position=native_owner.base.xyz(u,p+0x9C),on_bridge=u.mem_read(p+0x8C,1)[0],loco_head=native_owner.base.xyz(u,m.read32(p+0x674)+FAMILIES['walk' if m.read32(p)==0x7EB058 else 'drive']['head_offset']),firing=u.mem_read(p+0x68D,1)[0],scan=u.mem_read(p+0x688,1)[0],
   visit=m.read32(p+0xC4),targeting_timer=list(struct.unpack('<3i',u.mem_read(p+0x180,12))),idle_timer=list(struct.unpack('<3i',u.mem_read(p+0x168,12))),
   doing=native_owner.base.i32(u,p+0x6C4) if m.read32(p)==0x7EB058 else None)

 def initial_action_snap(self,p):
  m,u=self.m,self.u;interface=m.read32(p+0x674)
  return dict(actor=hex(p),vtable=hex(m.read32(p)),doing=native_owner.base.i32(u,p+0x6C4),
   mission=native_owner.base.i32(u,p+0xAC),queued=native_owner.base.i32(u,p+0xB4),
   alive=u.mem_read(p+0x90,1)[0],limbo=u.mem_read(p+0x81,1)[0],position=native_owner.base.xyz(u,p+0x9C),on_bridge=u.mem_read(p+0x8C,1)[0],
   walk_interface=hex(interface),walk_vtable=hex(m.read32(interface)),walk_moving_getter=hex(m.read32(m.read32(interface)+0x10)),walk_moving=u.mem_read(interface+0x30,1)[0],
   walk_destination=native_owner.base.xyz(u,interface+0x18),walk_head=native_owner.base.xyz(u,interface+0x24),
   speed_fraction_bits=bytes(u.mem_read(p+0x578,8)).hex(),frame_f8=native_owner.base.i32(u,p+0xF8),
   sequence_timer_words=list(struct.unpack('<4i',u.mem_read(p+0x100,16))),body_facing_words=list(struct.unpack('<6I',u.mem_read(p+0x388,24))))

 def probe_initial_action(self,ctor):
  # Execute before this companion's explicit Ready; restore all native writes,
  # CPU/x87, heap cursor and inherited observations to preserve existing rows.
  m,u=self.m,self.u;p=self.e1;before=self.initial_action_snap(p)
  rng_before={key:native_owner.base.sr.rng_state(u,ptr) for key,ptr in self.resident.rngs.items()}
  cpu=u.context_save();cursor=m.cursor;phase=self.phase;inherited_events=list(self.events);inherited_pending=dict(self.pending)
  events=[];writes=[];journal=[];returns={};instruction=[0]
  entries={0x520AE0:('sequencer',0),0x75AB30:('walk_is_moving',1),0x51D6F0:('do_action',3),0x4C9300:('primary_facing',1),0x65C7E0:('rng_ranged',2),0x65C780:('rng_next',0)}
  def observe(uc,a,n,d):
   instruction[0]+=1;sp=uc.reg_read(UC_X86_REG_ESP)
   if a in returns:
    for event in returns.pop(a):event['returned_eax']=uc.reg_read(UC_X86_REG_EAX);event['returned_al']=uc.reg_read(UC_X86_REG_EAX)&255
   if a in entries:
    kind,count=entries[a];args=[m.read32(sp+4+i*4) for i in range(count)]
    event=dict(instruction=instruction[0],pc=hex(a),kind=kind,caller=hex(m.read32(sp)),this=hex(args[0] if a==0x75AB30 else uc.reg_read(UC_X86_REG_ECX)),convention='COM_stdcall' if a==0x75AB30 else 'thiscall',args=args)
    events.append(event);returns.setdefault(m.read32(sp),[]).append(event)
  fields={0x6C4:'doing',0xF8:'frame_f8',0x100:'sequence_start',0x104:'sequence_aux',0x108:'sequence_duration',0x10C:'sequence_copy'}
  def written(uc,access,address,size,value,d):
   journal.append((address,bytes(uc.mem_read(address,size))))
   if address-p in fields:writes.append(dict(instruction=instruction[0],field=fields[address-p],pc=hex(uc.reg_read(UC_X86_REG_EIP)),offset=hex(address-p),bytes=size,value=value))
  self.phase='logic';self.events.clear();self.pending.clear()
  h=u.hook_add(UC_HOOK_CODE,observe);w=u.hook_add(UC_HOOK_MEM_WRITE,written)
  try:
   answer=m.invoke(0x520AE0,p)
   result=dict(entry='0x00520AE0',constructor=ctor,before=before,after=self.initial_action_snap(p),rng_before=rng_before,
    rng_after={key:native_owner.base.sr.rng_state(u,ptr) for key,ptr in self.resident.rngs.items()},returned_eax=answer,
    events=events,writes=writes,instruction_count=instruction[0],callback_events=list(self.events))
   result['rng_unchanged']=result['rng_before']==result['rng_after']
   result['native_fpcw']=u.reg_read(UC_X86_REG_FPCW)
   result['native_frame']=m.read32(0xA8ED84)
   assert m.cursor==cursor,'unexpected allocation in bounded stationary initial-action control'
   assert hashlib.sha256(bytes(u.mem_read(0x401000,0x3E0000))).hexdigest()==self.resident.code_hash
  finally:
   u.hook_del(h);u.hook_del(w)
   for address,previous in reversed(journal):u.mem_write(address,previous)
   u.context_restore(cpu);self.phase=phase;self.events[:]=inherited_events;self.pending.clear();self.pending.update(inherited_pending)
  assert before==self.initial_action_snap(p)
  assert rng_before=={key:native_owner.base.sr.rng_state(u,ptr) for key,ptr in self.resident.rngs.items()}
  result['native_state_restored_before_explicit_ready']=True
  return result

 def locomotor_readback(self,p):
  m,u=self.m,self.u;interface=m.read32(p+0x674);vtable=m.read32(interface)
  getter=m.read32(vtable+0x10);cpu=u.context_save();stack=bytes(u.mem_read(SP,16));journal=[]
  def written(uc,access,address,size,value,d):journal.append((address,bytes(uc.mem_read(address,size))))
  hook=u.hook_add(UC_HOOK_MEM_WRITE,written)
  try:answer=m.invoke(getter,0,(interface,))&255
  finally:
   u.hook_del(hook)
   for address,previous in reversed(journal):u.mem_write(address,previous)
   u.mem_write(SP,stack);u.context_restore(cpu)
  family='walk' if m.read32(p)==0x7EB058 else 'drive'
  return dict(family=family,interface=hex(interface),vtable=hex(vtable),move_to=hex(m.read32(vtable+0x44)),
   is_moving_getter=hex(getter),is_moving_al=answer,
   destination=native_owner.base.xyz(u,interface+(0x18 if family=='walk' else 0x30)),
   head=native_owner.base.xyz(u,interface+FAMILIES[family]['head_offset']))

 def navigation_snap(self,p,queue_entries):
  m,u=self.m,self.u;state=self.snap(p);queue=m.read32(p+0x58C)
  state.update(locomotor=self.locomotor_readback(p),aux=hex(m.read32(p+0x5A0)),
   path=list(struct.unpack('<24i',u.mem_read(p+0x5E0,96))),reference_cell=list(struct.unpack('<2h',u.mem_read(p+0x558,4))),
   nav_queue=dict(vtable=hex(m.read32(p+0x588)),backing=hex(queue),capacity=m.read32(p+0x590),initialized=u.mem_read(p+0x594,1)[0],storage_owned=u.mem_read(p+0x595,1)[0],count=m.read32(p+0x598),entries=[hex(m.read32(queue+i*4)) for i in range(queue_entries)]),
   movement_timer_words=list(struct.unpack('<3i',u.mem_read(p+0x640,12))),blocked_timer_words=list(struct.unpack('<3i',u.mem_read(p+0x668,12))),
   blocked=u.mem_read(p+0x6B7,1)[0],retry=native_owner.base.i32(u,p+0x64C),frame_f8=native_owner.base.i32(u,p+0xF8),
   sequence_timer_words=list(struct.unpack('<4i',u.mem_read(p+0x100,16))),body_facing_words=list(struct.unpack('<6I',u.mem_read(p+0x388,24))))
  return state

 def cell_readback(self,p):
  u=self.u
  return dict(pointer=hex(p),cell=list(struct.unpack('<2h',u.mem_read(p+0x24,4))),level=struct.unpack('<b',u.mem_read(p+0x11B,1))[0],slope=u.mem_read(p+0x11C,1)[0],flags=self.m.read32(p+0x140),land=native_owner.base.i32(u,p+0xEC))

 def dummy_snap(self):return self.cell_readback(0xABDC50)

 def idle_snap(self,p):
  m,u=self.m,self.u;state=self.snap(p);interface=m.read32(p+0x674)
  state.update(primary_facing_words=list(struct.unpack('<6I',u.mem_read(p+0x388,24))),
   frame_f8=native_owner.base.i32(u,p+0xF8),prone_6db=u.mem_read(p+0x6DB,1)[0],
   sequence_timer_words=list(struct.unpack('<4i',u.mem_read(p+0x100,16))),
   walk_moving=dict(interface=hex(interface),vtable=hex(m.read32(interface)),getter=hex(m.read32(m.read32(interface)+0x10)),field=hex(interface+0x30),value=u.mem_read(interface+0x30,1)[0]))
  return state

 def row(self,name,family='MTNK',mission=21,status=2,target=0,archive=0,nav=0,seed=31,stop=None,scan_expired=False,idle_expired=False,candidate_result=None,candidate_xyz=None,archive_xyz=None,source_xyz=None,on_bridge=None,head_xyz=None,source_cell_flags=None,archive_flags=None,candidate_live=False,dispatch_entry=False,greatest_args=None,idle_args=None,navigation_args=None,home_args=None,evidence_context=None,**flags):
  m,u=self.m,self.u;p=self.src if family=='MTNK' else self.e1
  u.mem_write(p,self.baselines[p])
  local=m.read32(p+0x674)-4;u.mem_write(local,self.loco_baselines[local])
  # Candidate controls must follow this restore of their placed, live baselines.
  for ptr,raw in self.prototype_baselines.items():u.mem_write(ptr,raw)
  for ptr,raw in self.cell_baselines.items():u.mem_write(ptr,raw)
  u.mem_write(self.candidate+0x81,bytes([not candidate_live]))
  if source_xyz is not None:u.mem_write(p+0x9C,dwords(*source_xyz))
  if on_bridge is not None:u.mem_write(p+0x8C,bytes([on_bridge]))
  if head_xyz is not None:u.mem_write(m.read32(p+0x674)+FAMILIES['walk' if family=='E1' else 'drive']['head_offset'],dwords(*head_xyz))
  if archive_flags is not None:u.mem_write(self.victim+0x14,dwords(archive_flags))
  if source_cell_flags is not None:
   xyz=native_owner.base.xyz(u,p+0x9C);xy=(int(xyz[0]/256),int(xyz[1]/256));u.mem_write(self.resident.ptrs[xy]+0x140,dwords(source_cell_flags))
  if greatest_args is not None:
   for label,ptr in self.extra_candidates.items():u.mem_write(ptr+0x81,bytes([not greatest_args['extra_live'].get(label,False)]))
  if candidate_xyz is not None:u.mem_write(self.candidate+0x9C,dwords(*candidate_xyz))
  if archive_xyz is not None:u.mem_write(self.victim+0x9C,dwords(*archive_xyz))
  if idle_args is not None:
   if idle_args['entry']!='completion':u.mem_write(p+0x6C4,dwords(idle_args['doing']))
   if 'prone' in idle_args:u.mem_write(p+0x6DB,bytes([idle_args['prone']]))
   if 'frame_f8' in idle_args:u.mem_write(p+0xF8,dwords(idle_args['frame_f8']))
   if 'loco_moving' in idle_args:u.mem_write(m.read32(p+0x674)+0x30,bytes([idle_args['loco_moving']]))
  if navigation_args is not None:
   for ptr in navigation_args['inactive_registered_candidates']:
    u.mem_write(ptr+0x81,bytes([1]));u.mem_write(ptr+0x90,bytes([0]))
   assert len(navigation_args['path'])==24
   u.mem_write(p+0x5E0,dwords(*navigation_args['path']));u.mem_write(p+0x558,struct.pack('<2h',*navigation_args['reference_cell']))
   u.mem_write(p+0x58C,dwords(navigation_args['queue_backing'],len(navigation_args['queue_entries'])))
   u.mem_write(p+0x598,dwords(len(navigation_args['queue_entries'])))
   u.mem_write(navigation_args['queue_backing'],dwords(*navigation_args['queue_entries']))
   u.mem_write(p+0x5A0,dwords(navigation_args['aux']))
   u.mem_write(p+0x640,dwords(*navigation_args['movement_timer_words']));u.mem_write(p+0x668,dwords(*navigation_args['blocked_timer_words']))
   u.mem_write(p+0x64C,dwords(navigation_args['retry']));u.mem_write(p+0x6B7,bytes([navigation_args['blocked']]))
  if home_args is not None:
   u.mem_write(self.house+0x5490,struct.pack('<2h',*home_args['primary']));u.mem_write(self.house+0x5494,struct.pack('<2h',*home_args['alternate']))
   u.mem_write(self.house+0x5498,dwords(home_args['radius']))
  # Explicit prior mission state/placement continuation, not a constructor claim.
  for off,value in [(0xAC,mission),(0xB4,-1),(0xBC,status),(0x2B4,target),(0x218,archive),(0x5A4,nav),(0xC8,0),(0xD0,0)]:u.mem_write(p+off,dwords(value))
  if scan_expired:u.mem_write(p+0x180,dwords(-1,0,0))
  if idle_expired:u.mem_write(p+0x168,dwords(-1,0,0))
  if idle_args is not None and 'timer_words' in idle_args:u.mem_write(p+0x168,dwords(*idle_args['timer_words']))
  for key,value in flags.items():
   off={'firing':0x68D,'scan':0x688,'moving':0x3D5}[key];u.mem_write(p+off,bytes([value]))
  scenario=m.read32(0xA8B230)+0x218;m.invoke(0x65C6D0,scenario,(seed,))
  start=self.snap(p);rng0={k:native_owner.base.sr.rng_state(u,p) for k,p in self.resident.rngs.items()}
  u.mem_write(SP-0x1000,bytes(0x2000))
  self.events.clear();self.pending.clear();events=[];writes=[];visited=set();returns={};last=deque(maxlen=30);journal=[];instruction=[0]
  names={0x5B3060:'dispatch',0x4DBDF0:'navigation_coordinate',0x4DDC40:'bridge_layer',0x4DDF90:'rescue',0x4D6AA0:'area_guard',0x5B35E0:'queue',0x5B3570:'commence',0x5B3A00:'mission_control',
   0x70C610:'archive',0x743190:'unit_greatest',0x51E140:'infantry_greatest',0x6F8DF0:'greatest',0x70CD10:'score',0x6F7CA0:'evaluate',0x4D9920:'foot_greatest',0x709820:'retaliate_scan',
   0x707E60:'threat_range',0x7414E0:'unit_approach',0x522340:'infantry_approach',0x4D5690:'foot_approach',
   0x6FCDB0:'assign_target',0x51B1F0:'infantry_assign_target',0x741970:'unit_destination',0x51AA40:'infantry_destination',
   0x4D94B0:'foot_destination',0x565730:'xyz_cell',0x5657A0:'cell_lookup',0x5F6360:'distance',0x65C7E0:'rng',0x65C780:'rng_next',0x65C640:'rng_raw_init',0x65C660:'rng_control',
   0x51D6F0:'infantry_action',0x7099E0:'idle_admission',0x41C040:'unit_idle',0x51CDB0:'infantry_idle',0x7091D0:'can_acquire',0x70F7E0:'targeting_timer'}
  counts={0x5B35E0:2,0x70C610:1,0x743190:3,0x51E140:3,0x6F8DF0:3,0x707E60:1,
   0x7414E0:1,0x522340:1,0x4D5690:1,0x6FCDB0:1,0x51B1F0:1,0x741970:2,0x51AA40:2,0x4D94B0:2,
   0x565730:1,0x5657A0:1,0x5F6360:1,0x65C7E0:2,0x70CD10:2,0x6F7CA0:7,0x4D9920:3,0x709820:2,0x51D6F0:3}
  for address in self.entry_getcoords:names[address]='physical_getcoords';counts[address]=1
  if idle_args is not None:
   names.update({0x4C9300:'primary_facing',0x5F3E50:'literal_type_check',0x5FB2E0:'game_options_delay',0x5216D0:'infantry_idle_admission',0x75AB30:'walk_is_moving',0x520AE0:'infantry_sequencer'});counts.update({0x4C9300:1,0x5F3E50:1,0x5FB2E0:1,0x75AB30:1})
   start=self.idle_snap(p)
  if navigation_args is not None:
   names.update({0x4AFD40:'drive_move_to',0x75ACB0:'walk_move_to',0x4E0190:'clear_nav_queue'});counts.update({0x4AFD40:4,0x75ACB0:4})
   start=self.navigation_snap(p,len(navigation_args['queue_entries']))
   destination_input_readback=dict(archive_object=None if not archive else self.snap(archive),archive_locomotor=None if not archive else self.locomotor_readback(archive),rules_repath_delay=native_owner.base.i32(u,self.rules+0x1768),frame=m.read32(0xA8ED84))
   counts.update({0x4DBDF0:2,0x4DDC40:1})
  if home_args is not None:
   names.update({0x500200:'house_return',0x501AC0:'house_random_return',0x50DEF0:'house_base_center',0x49F420:'random_direction',0x578460:'inside_map',0x586E50:'clamp_map',0x56D230:'zone',0x56DC20:'nearby_cell'})
   counts.update({0x500200:2,0x501AC0:2,0x50DEF0:1,0x49F420:2,0x578460:2,0x586E50:2,0x56D230:3,0x56DC20:15})
   home_before=self.dummy_snap()
  if evidence_context is not None:
   registration_before=self.registration()
   candidate_liveness_before=[dict(pointer=pointer,limbo=u.mem_read(int(pointer,16)+0x81,1)[0],alive=u.mem_read(int(pointer,16)+0x90,1)[0],xyz=native_owner.base.xyz(u,int(pointer,16)+0x9C),house=hex(m.read32(int(pointer,16)+0x21C))) for pointer in registration_before['techno']['actors']]
  def observe(uc,a,n,d):
   instruction[0]+=1;visited.add(a);last.append(a);sp=uc.reg_read(UC_X86_REG_ESP)
   if a in returns:
    for row in returns.pop(a):
     row['result_eax']=uc.reg_read(UC_X86_REG_EAX)
     if row['kind']=='physical_getcoords':row['xyz']=native_owner.base.xyz(uc,uc.reg_read(UC_X86_REG_EAX))
     if row['kind']=='mission_control':row['rate_bits']=bytes(uc.mem_read(uc.reg_read(UC_X86_REG_EAX)+0x10,8)).hex()
     if row['kind']=='evaluate':row['score_result']=native_owner.base.i32(uc,row['args'][4])
     if '_cell_output' in row:row['output_cell']=list(struct.unpack('<2h',uc.mem_read(row.pop('_cell_output'),4)))
     if '_coord_output' in row:row['output_coordinate']=native_owner.base.xyz(uc,row.pop('_coord_output'))
     if navigation_args is not None and row['kind'] in ('unit_destination','infantry_destination'):row['rng_at_return']={key:native_owner.base.sr.rng_state(uc,ptr) for key,ptr in self.resident.rngs.items()}
     if navigation_args is not None and row['kind']=='navigation_coordinate':row['output_coordinate']=native_owner.base.xyz(uc,uc.reg_read(UC_X86_REG_EAX))
     if (navigation_args is not None or home_args is not None) and row['kind'] in ('xyz_cell','cell_lookup'):row['resolved_cell']=self.cell_readback(uc.reg_read(UC_X86_REG_EAX))
     if idle_args is not None and row['kind']=='infantry_idle':row['rng_at_return']={key:native_owner.base.sr.rng_state(uc,ptr) for key,ptr in self.resident.rngs.items()}
   if a in names:
    row=dict(instruction=instruction[0],kind=names[a],pc=hex(a),caller=hex(m.read32(sp)),this=hex(uc.reg_read(UC_X86_REG_ECX)),
     args=[m.read32(sp+4+4*i) for i in range(counts.get(a,0))]);events.append(row);returns.setdefault(m.read32(sp),[]).append(row)
    if a in (0x65C640,0x65C660,0x65C780,0x65C7E0):row['stream']=next((k for k,p in self.resident.rngs.items() if p==uc.reg_read(UC_X86_REG_ECX)),hex(uc.reg_read(UC_X86_REG_ECX)))
    if idle_args is not None and a==0x4C9300:row['desired_u16']=struct.unpack('<H',uc.mem_read(row['args'][0],2))[0]
    if idle_args is not None and a==0x5F3E50:row['literal']=m.string(row['args'][0])
    if a==0x565730:row['xyz']=native_owner.base.xyz(uc,row['args'][0])
    if a==0x5657A0:row['cell']=list(struct.unpack('<2h',uc.mem_read(row['args'][0],4)))
    if navigation_args is not None and a in (0x4AFD40,0x75ACB0):row.update(interface=hex(row['args'][0]),coordinate=[native_owner.base.i32(uc,sp+8+i*4) for i in range(3)],convention='COM_stdcall')
    if navigation_args is not None and a in (0x741970,0x51AA40):row['rng_at_entry']={key:native_owner.base.sr.rng_state(uc,ptr) for key,ptr in self.resident.rngs.items()}
    if home_args is not None:
     row['dummy']=self.dummy_snap()
     if a in (0x500200,0x501AC0,0x50DEF0,0x56DC20,0x586E50):row['_cell_output']=row['args'][0]
     if a==0x501AC0:row.update(variant=row['args'][1],radius=native_owner.base.i32(uc,self.house+0x5498))
     if a==0x56D230:row['cell']=list(struct.unpack('<2h',uc.mem_read(row['args'][0],4)))
     if a==0x56DC20:row.update(seed_cell=list(struct.unpack('<2h',uc.mem_read(row['args'][1],4))),target_cell=list(struct.unpack('<2h',uc.mem_read(row['args'][12],4))))
     if a==0x49F420:row.update(current_coordinate=native_owner.base.xyz(uc,uc.reg_read(UC_X86_REG_EDX)),_coord_output=uc.reg_read(UC_X86_REG_ECX))
     if a in (0x578460,0x586E50):row['cell']=list(struct.unpack('<2h',uc.mem_read(row['args'][0 if a==0x578460 else 1],4)))
    if a==0x70CD10:row['anchor_xyz']=native_owner.base.xyz(uc,row['args'][1])
    if a==0x6F7CA0:row['anchor_xyz']=native_owner.base.xyz(uc,row['args'][6])
    if a in (0x743190,0x51E140,0x6F8DF0,0x4D9920):row['anchor_xyz']=native_owner.base.xyz(uc,m.read32(sp+8))
   if greatest_args is not None and a==0x6F9C86:events.append(dict(instruction=instruction[0],kind='global_candidate_visit',pc=hex(a),index=uc.reg_read(UC_X86_REG_EBX),candidate=hex(m.read32(uc.reg_read(UC_X86_REG_EDX)+uc.reg_read(UC_X86_REG_EBX)*4))))
   if greatest_args is not None and a in (0x6F9D87,0x6F9D8B):events.append(dict(instruction=instruction[0],kind='global_score_compare' if a==0x6F9D87 else 'global_best_update',pc=hex(a),candidate=hex(uc.reg_read(UC_X86_REG_EDI)),score=uc.reg_read(UC_X86_REG_EAX),previous_score=uc.reg_read(UC_X86_REG_ECX)))
   if a==0x6F8710:events.append(dict(instruction=instruction[0],kind='score_before_vhp',pc=hex(a),eax=uc.reg_read(UC_X86_REG_EAX)))
   if a in (0x65C84B,0x65C79D):events.append(dict(instruction=instruction[0],kind='rng_raw',pc=hex(a),value=uc.reg_read(UC_X86_REG_ESI)))
   if home_args is not None and a==0x65C7D0:events.append(dict(instruction=instruction[0],kind='direction_rng_raw',pc=hex(a),value=uc.reg_read(UC_X86_REG_EAX)))
   if home_args is not None and a==0x501B15:events.append(dict(instruction=instruction[0],kind='house_clamped_radius',pc=hex(a),radius=uc.reg_read(UC_X86_REG_EAX)))
   if a in (0x4D6E64,0x4DE0DB,0x4DE1AF,0x4D703A,0x4D712E):events.append(dict(instruction=instruction[0],kind='math',pc=hex(a),eax=uc.reg_read(UC_X86_REG_EAX)))
   if a==0x4D6E8F:events.append(dict(instruction=instruction[0],kind='chosen_leash',pc=hex(a),edi=uc.reg_read(UC_X86_REG_EDI)))
   if a in (0x70CFBE,0x70D023,0x70D0A0):events.append(dict(instruction=instruction[0],kind='score_anchor_branch',pc=hex(a),eax=uc.reg_read(UC_X86_REG_EAX)))
  fields={p+off:label for off,label in [(0xAC,'mission'),(0xB4,'queued'),(0xBC,'status'),(0x218,'archive'),(0x2B4,'target'),(0x5A4,'nav'),(0xC8,'timer_frame'),(0xD0,'timer_duration'),(0x688,'scan'),(0x168,'idle_start'),(0x170,'idle_delay'),(0x180,'targeting_start'),(0x188,'targeting_delay'),(0x6C4,'doing')]}
  if idle_args is not None:
   fields.update({p+offset:f'primary_facing_{offset-0x388:x}' for offset in range(0x388,0x3A0,4)})
   fields.update({p+offset:label for offset,label in ((0xF8,'sequence_frame_f8'),(0x100,'sequence_start'),(0x104,'sequence_aux'),(0x108,'sequence_duration'),(0x10C,'sequence_copy'))})
  if navigation_args is not None:
   fields.update({p+offset:label for offset,label in ((0x5A0,'aux'),(0x5E0,'path_head'),(0x598,'nav_queue_count'),(0x640,'movement_start'),(0x644,'movement_aux'),(0x648,'movement_delay'),(0x668,'blocked_start'),(0x66C,'blocked_aux'),(0x670,'blocked_delay'),(0x6B7,'blocked'),(0xF8,'sequence_frame_f8'),(0x100,'sequence_start'),(0x104,'sequence_aux'),(0x108,'sequence_duration'),(0x10C,'sequence_copy'))})
   interface=m.read32(p+0x674);destination=interface+(0x18 if family=='E1' else 0x30)
   fields.update({destination+i*4:f'locomotor_destination_{i}' for i in range(3)})
  dummy_writes=[]
  def record(uc,access,a,n,value,d):
   journal.append((a,bytes(uc.mem_read(a,n))))
   if a in fields:writes.append(dict(instruction=instruction[0],field=fields[a],pc=hex(uc.reg_read(UC_X86_REG_EIP)),value=value,bytes=n))
   if home_args is not None and a==0xABDC74:dummy_writes.append(dict(instruction=instruction[0],pc=hex(uc.reg_read(UC_X86_REG_EIP)),bytes=n,cell=list(struct.unpack('<2h',dwords(value)))))
  h=u.hook_add(UC_HOOK_CODE,observe);w=u.hook_add(UC_HOOK_MEM_WRITE,record)
  try:
   entry=0x4DDF90 if mission==21 else (0x744100 if family=='MTNK' else 0x51F640)
   if dispatch_entry:entry=0x5B3060
   completion_setup=None
   if idle_args is not None and idle_args['entry']=='idle':entry=0x51CDB0
   if idle_args is not None and idle_args['entry']=='completion':
    entry=0x520AE0;setup_before=self.idle_snap(p);setup_rng_before=rng0
    admitted=m.invoke(0x51D6F0,p,(idle_args['doing'],0,0))&255;assert admitted
    table=m.read32(self.e1_type+0xE3C);sequence=table+idle_args['doing']*0x24;count=native_owner.base.i32(u,sequence+4)
    completion_setup=dict(entry='0x0051D6F0',action=idle_args['doing'],returned_al=admitted,before=setup_before,after=self.idle_snap(p),rng_before=setup_rng_before,rng_after={key:native_owner.base.sr.rng_state(u,ptr) for key,ptr in self.resident.rngs.items()},sequence_table=hex(table),sequence=hex(sequence),sequence_bytes=bytes(u.mem_read(sequence,0x24)).hex(),native_frame_count=count)
    u.mem_write(p+0xF8,dwords(count));start=self.idle_snap(p);rng0=completion_setup['rng_after']
    events.append(dict(instruction=instruction[0],kind='supplied_completion_stage',frame_f8=count))
   if greatest_args is not None:
    entry=0x4D9920 if greatest_args['entry']=='foot' else m.read32(m.read32(p)+0x3C4)
    u.mem_write(self.coord,dwords(*greatest_args['point_xyz']))
   if candidate_result is not None:
    u.mem_write(SP,dwords(RET_MAGIC));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,p)
    run_checked(u,entry,0x4DE056,count=2000000)
    sp=u.reg_read(UC_X86_REG_ESP)
    prefix=dict(pc='0x004DE056',args=[m.read32(sp+4*i) for i in range(3)],anchor_xyz=native_owner.base.xyz(u,m.read32(sp+4)),
     virtual_receiver=hex(u.reg_read(UC_X86_REG_ECX)),vtable=hex(m.read32(p)),scanner=hex(m.read32(m.read32(p)+0x3C4)))
    events.append(dict(instruction=instruction[0],kind='supplied_scanner_return',**prefix,input_result=hex(candidate_result)))
    u.reg_write(UC_X86_REG_ESP,sp+12);u.reg_write(UC_X86_REG_EAX,candidate_result)
    end=run_checked(u,0x4DE05C,(RET_MAGIC,0x4DE12D),count=2000000);answer=u.reg_read(UC_X86_REG_EAX)
   elif stop is None:answer=m.invoke(entry,p,() if greatest_args is None else (greatest_args['mask'],self.coord,greatest_args['aux_bool']));end=RET_MAGIC
   else:
    u.mem_write(SP,dwords(RET_MAGIC));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,p)
    end=run_checked(u,entry,stop,count=2000000);answer=u.reg_read(UC_X86_REG_EAX)
  except Exception as e:
   print('FAILED',name,type(e).__name__,str(e),'last',[hex(a) for a in last],self.snap(p),flush=True);raise
  finally:u.hook_del(h);u.hook_del(w)
  result=dict(input=dict(name=name,family=family,entry=hex(entry),mission=mission,status=status,target=hex(target),archive=hex(archive),nav=hex(nav),flags=flags,scenario_seed=seed,scan_expired=scan_expired,idle_expired=idle_expired,
    supplied_scanner_result=None if candidate_result is None else hex(candidate_result),candidate_xyz=candidate_xyz,archive_xyz=archive_xyz,source_xyz=source_xyz,on_bridge=on_bridge,head_xyz=head_xyz,source_cell_flags=source_cell_flags,archive_flags=archive_flags,candidate_live=candidate_live,dispatch_entry=dispatch_entry),
   before=start,returned_eax=answer,after=self.snap(p),rng_before=rng0,rng_after={k:native_owner.base.sr.rng_state(u,p) for k,p in self.resident.rngs.items()},events=events,writes=writes,
   stop=hex(end),returned=end==RET_MAGIC,original_rescue_visited=0x4DDF90 in visited,original_area_guard_visited=0x4D6AA0 in visited,navigation_coordinate_calls=sum(e['kind']=='navigation_coordinate' for e in events),bridge_layer_calls=sum(e['kind']=='bridge_layer' for e in events),inherited_rng_events=[e for e in self.events if e['kind'] in ('rng','raw')],callback_events=[e for e in self.events if any(k in e['kind'] for k in ('boundary','OS_','COM_'))],instruction_count=instruction[0])
  if idle_args is not None:
   result['input']['idle_control']=dict(**idle_args,idle_action_frequency_bits=bytes(u.mem_read(self.rules+0x1710,8)).hex(),native_fpcw=u.reg_read(UC_X86_REG_FPCW))
   result['after']=self.idle_snap(p)
   if idle_args['entry']=='idle':result['returned_al']=answer&255
   if completion_setup is not None:result['completion_setup']=completion_setup
  if greatest_args is not None:
   result['input']['greatest_threat']=greatest_args
   result['greatest_threat_masks']={kind:[e['args'][0] for e in events if e['kind']==kind] for kind in ('unit_greatest','infantry_greatest','foot_greatest','greatest')}
  if navigation_args is not None:
   result['input']['navigation_control']=navigation_args;result['after']=self.navigation_snap(p,len(navigation_args['queue_entries']))
   result['destination_input_readback']=destination_input_readback
   result['inactive_candidate_readback']=[dict(pointer=hex(ptr),limbo=u.mem_read(ptr+0x81,1)[0],alive=u.mem_read(ptr+0x90,1)[0]) for ptr in navigation_args['inactive_registered_candidates']]
  if home_args is not None:
   result['input']['house_return_control']=home_args
   result['house_return']=dict(before_dummy=home_before,after_dummy=self.dummy_snap(),dummy_writes=dummy_writes,
    map_size_words=list(struct.unpack('<6i',u.mem_read(native_owner.base.sr.MAP+0xF4,24))),zone_storage=self.resident.zone_storage)
  if evidence_context is not None:
   result['input'].update(evidence_context)
   result.update(registration_before=registration_before,candidate_liveness_before=candidate_liveness_before)
  for address,previous in reversed(journal):u.mem_write(address,previous)
  assert hashlib.sha256(bytes(u.mem_read(0x401000,0x3E0000))).hexdigest()==self.resident.code_hash
  assert all(bytes(u.mem_read(int(a,16),0x600)).hex()==raw for a,raw in self.vtables.items())
  return result

 def e1_weapon_state(self):
  """Observe actual normal/elite slots and execute shared native range getters."""
  m,u=self.m,self.u;slots=[]
  def named(p):return dict(pointer=hex(p),name=m.string(p+0x24))
  for tier,entry in (('normal',0x7177C0),('elite',0x7177E0)):
   for index in (0,1):
    slot=m.invoke(entry,self.e1_type,(index,));p=m.read32(slot);weapon=None
    if p:
     projectile=m.read32(p+0xA0);warhead=m.read32(p+0xAC)
     weapon=dict(**named(p),range_leptons=native_owner.base.i32(u,p+0xB4),minimum_range_leptons=native_owner.base.i32(u,p+0xB8),
      damage=native_owner.base.i32(u,p+0xA4),rof=native_owner.base.i32(u,p+0xB0),burst=native_owner.base.i32(u,p+0x9C),speed=native_owner.base.i32(u,p+0xA8),
      projectile=None if not projectile else dict(**named(projectile),**m.bullet_state(projectile),
       subject_to_cliffs=u.mem_read(projectile+0x296,1)[0],subject_to_elevation=u.mem_read(projectile+0x297,1)[0],subject_to_walls=u.mem_read(projectile+0x298,1)[0]),
      warhead=None if not warhead else dict(**named(warhead),verses_bits=[f'{v:016x}' for v in struct.unpack('<11Q',u.mem_read(warhead+0xA0,88))],
       prone_damage_bits=bytes(u.mem_read(warhead+0xF8,8)).hex(),cell_spread_bits=bytes(u.mem_read(warhead+0x124,4)).hex(),
       percent_at_max_bits=bytes(u.mem_read(warhead+0x12C,4)).hex(),wall=u.mem_read(warhead+0x144,1)[0],wood=u.mem_read(warhead+0x147,1)[0]))
    slots.append(dict(tier=tier,index=index,slot=hex(slot),type_get_weapon_entry=hex(entry),weapon=weapon))
  vtable=m.read32(self.e1);range_entry=m.read32(vtable+0x168);weapon_entry=m.read32(vtable+0x3F8)
  assert (range_entry,weapon_entry)==(0x7012C0,0x70E140)
  events=[];returns={};rng_before={key:native_owner.base.sr.rng_state(u,ptr) for key,ptr in self.resident.rngs.items()}
  def observe(uc,a,n,d):
   sp=uc.reg_read(UC_X86_REG_ESP)
   if a in returns:
    for event in returns.pop(a):event['returned_eax']=uc.reg_read(UC_X86_REG_EAX)
   if a in (0x7012C0,0x70E140,0x7177C0,0x7177E0,0x707E60):
    event=dict(pc=hex(a),caller=hex(m.read32(sp)),this=hex(uc.reg_read(UC_X86_REG_ECX)),argument=native_owner.base.i32(uc,sp+4));events.append(event);returns.setdefault(m.read32(sp),[]).append(event)
  hook=u.hook_add(UC_HOOK_CODE,observe)
  veterancy=bytes(u.mem_read(self.e1+0x150,4));controls=[]
  try:
   ranges=[m.invoke(range_entry,self.e1,(index,)) for index in (0,1)]
   threats=[m.invoke(0x707E60,self.e1,(mode,)) for mode in (0,1,2)]
   controls.append(dict(name='retained_rookie',supplied=False,veterancy_bits=veterancy.hex(),weapon_range_leptons=ranges,threat_range_leptons=threats))
   u.mem_write(self.e1+0x150,struct.pack('<f',2.0))
   controls.append(dict(name='supplied_elite',supplied=True,veterancy_bits=bytes(u.mem_read(self.e1+0x150,4)).hex(),
    weapon_range_leptons=[m.invoke(range_entry,self.e1,(index,)) for index in (0,1)],threat_range_leptons=[m.invoke(0x707E60,self.e1,(mode,)) for mode in (0,1,2)]))
  finally:u.mem_write(self.e1+0x150,veterancy);u.hook_del(hook)
  rng_after={key:native_owner.base.sr.rng_state(u,ptr) for key,ptr in self.resident.rngs.items()}
  assert rng_before==rng_after
  return dict(slots=slots,veterancy_bits=bytes(u.mem_read(self.e1+0x150,4)).hex(),guard_range=native_owner.base.i32(u,self.e1_type+0x5B8),
   range_entry=hex(range_entry),get_weapon_entry=hex(weapon_entry),weapon_range_leptons=ranges,threat_range_leptons=threats,
   range_controls=controls,getter_events=events,rng_unchanged=True)

 def weapon_reader_receipts(self):
  # Run after every historical row. No old weapon, child or handler value is
  # retroactively changed by completing the omitted physical read context.
  m,u=self.m,self.u;self.phase='setup';before=self.e1_weapon_state()
  names={row['weapon']['name'] for row in before['slots'] if row['weapon']}
  # These are lexical reference names from the physical weapon sections.
  # Native readers decide references/scalars; no Rust export supplies a value.
  for _,path in native_owner.base.layers():
   if not path.exists():continue
   weapons,_=lexical(path.read_bytes(),names)
   for entries in weapons.values():
    for key in ('Projectile','Warhead'):
     if entries.get(key):names.add(entries[key])
  omitted=[]
  for row in before['slots']:
   weapon=row['weapon']
   if weapon and not any(read['weapon']['name']==weapon['name'] for read in self.e1_weapon_read_attempts):omitted.append(weapon['name'])
  for read in self.e1_weapon_read_attempts:
   for kind in ('projectile','warhead'):
    if not read[kind]['returned_al'] and read[kind]['name'] not in omitted:omitted.append(read[kind]['name'])
  # SSA belongs to an unread secondary and is physically present but was
  # never allocated/read by that weapon's constructor-only old slot.
  for _,path in native_owner.base.layers():
   if not path.exists():continue
   weapons,_=lexical(path.read_bytes(),set(omitted))
   for entries in weapons.values():
    for key in ('Projectile','Warhead'):
     if entries.get(key) and entries[key] not in omitted and not any(read[kind]['name']==entries[key] and read[kind]['returned_al'] for read in self.e1_weapon_read_attempts for kind in ('projectile','warhead')):omitted.append(entries[key])
  result=dict(omitted_sections=omitted,selected_sections=sorted(names),before=before,layers=[],historical_read_attempts=self.e1_weapon_read_attempts,
   reader_contexts=dict(rows='handler_rules_before_physical',initial_action_receipt='handler_rules_before_physical',
    retail_idle_rows='handler_rules_after_physical',greatest_threat_rows='handler_rules_after_physical',
    navigation_rows='handler_rules_after_physical',empty_rescue_rows='handler_rules_after_physical',retail_weapon_rows='handler_rules_after_physical'),
   historical_weapon_context='Only M60 receives WeaponType772080. InvisibleLow46BEE0 is attempted against an omitted section and retains native constructor fields. SA75D3A0 reads physical rules. Para/M60E/ParaE retain original771C70 constructor fields.',
   rules_context='handler_rules_after_physical',idle_action_frequency_bits=bytes(u.mem_read(self.rules+0x1710,8)).hex())
  active=[None];events=[];writes=[]
  def observe(uc,a,n,d):
   if a in (0x474620,0x5276D0,0x5283D0,0x528A10,0x5295F0):
    sp=uc.reg_read(UC_X86_REG_ESP)
    events.append(dict(pc=hex(a),caller=hex(m.read32(sp)),receiver=active[0],ini=hex(uc.reg_read(UC_X86_REG_ECX)),
     section=m.string(m.read32(sp+4)),key=m.string(m.read32(sp+8)),default_word=hex(m.read32(sp+12))))
  def record(uc,access,address,size,value,d):
   if active[0] is not None:
    p=int(active[0]['pointer'],16)
    if p<=address<p+active[0]['size']:writes.append(dict(pc=hex(uc.reg_read(UC_X86_REG_EIP)),receiver=active[0],offset=hex(address-p),bytes=size,value=value))
  h=u.hook_add(UC_HOOK_CODE,observe);w=u.hook_add(UC_HOOK_MEM_WRITE,record)
  try:
   for name,path in native_owner.base.layers():
    if not path.exists():
     assert name=='LANGRULE.INI';result['layers'].append(dict(file=name,absent=True));continue
    raw=path.read_bytes();sections,lines=lexical(raw,names);m.rules_cache(sections);events.clear();writes.clear()
    rng_before={key:native_owner.base.sr.rng_state(u,ptr) for key,ptr in self.resident.rngs.items()};reads=[]
    for p in dict.fromkeys(int(row['weapon']['pointer'],16) for row in before['slots'] if row['weapon']):
     active[0]=dict(kind='weapon',pointer=hex(p),name=m.string(p+0x24),size=0x160);wa=m.invoke(0x772080,p,(RULES,))&255
     projectile=m.read32(p+0xA0);warhead=m.read32(p+0xAC)
     active[0]=None if not projectile else dict(kind='projectile',pointer=hex(projectile),name=m.string(projectile+0x24),size=0x2F8)
     pa=m.invoke(0x46BEE0,projectile,(RULES,))&255 if projectile else None
     active[0]=None if not warhead else dict(kind='warhead',pointer=hex(warhead),name=m.string(warhead+0x24),size=0x1D0)
     ha=m.invoke(0x75D3A0,warhead,(RULES,))&255 if warhead else None
     active[0]=dict(kind='weapon_postpass',pointer=hex(p),name=m.string(p+0x24),size=0x160);m.invoke(0x7729F0,p)
     reads.append(dict(weapon=dict(entry='0x00772080',name=m.string(p+0x24),returned_al=wa),
      projectile=None if not projectile else dict(entry='0x0046BEE0',name=m.string(projectile+0x24),returned_al=pa),
      warhead=None if not warhead else dict(entry='0x0075D3A0',name=m.string(warhead+0x24),returned_al=ha),postpass='0x007729F0'))
    active[0]=None;rng_after={key:native_owner.base.sr.rng_state(u,ptr) for key,ptr in self.resident.rngs.items()}
    assert rng_before==rng_after
    result['layers'].append(dict(file=name,sha256=hashlib.sha256(raw).hexdigest(),sections=sections,source_lines=lines,
     read_attempts=reads,events=list(events),writes=list(writes),after=self.e1_weapon_state(),rng_before=rng_before,rng_after=rng_after,rng_unchanged=True))
  finally:u.hook_del(h);u.hook_del(w)
  result['after']=self.e1_weapon_state()
  assert hashlib.sha256(bytes(u.mem_read(0x401000,0x3E0000))).hexdigest()==self.resident.code_hash
  self.phase='logic';self.events.clear();self.pending.clear()
  return result

 def rules_reader_receipts(self):
  m,u=self.m,self.u;self.phase='setup';self.events.clear();self.pending.clear()
  fresh=m.alloc(0x2000)
  def state(pointer):
   return dict(guard_mode_stray=native_owner.base.i32(u,pointer+0x1724),idle_action_frequency_bits=bytes(u.mem_read(pointer+0x1710,8)).hex())
  def construct(pointer,guard_value):
   u.mem_write(pointer+0x1724,dwords(guard_value));before=state(pointer);native_fpcw=u.reg_read(UC_X86_REG_FPCW);writes=[]
   def record(uc,access,address,size,value,d):
    if pointer+0x1710<=address<pointer+0x1718 or address==pointer+0x1724:
     writes.append(dict(pc=hex(uc.reg_read(UC_X86_REG_EIP)),offset=hex(address-pointer),bytes=size,value=value))
   hook=u.hook_add(UC_HOOK_MEM_WRITE,record)
   try:returned=m.invoke(0x665650,pointer)
   finally:u.hook_del(hook)
   return dict(entry='0x00665650',receiver=hex(pointer),provided_guard_mode_stray=guard_value,returned_eax=hex(returned),before=before,after=state(pointer),writes=writes,native_fpcw=native_fpcw)
  result=dict(constructor=construct(fresh,0),constructor_retained_guard_control=construct(m.alloc(0x2000),0x12345678),controls=[],physical_layers=[])
  def read(name,pointer,sections,blocks,source=None):
   before=state(pointer);m.rules_cache(sections);events=[];visited=set();writes=[]
   def observe(uc,a,n,d):
    visited.add(a)
    if a in (0x474620,0x5283D0):
     sp=uc.reg_read(UC_X86_REG_ESP)
     event=dict(pc=hex(a),this=hex(uc.reg_read(UC_X86_REG_ECX)),section=m.string(m.read32(sp+4)),key=m.string(m.read32(sp+8)))
     event['default']=native_owner.base.i32(uc,sp+12) if a==0x474620 else bytes(uc.mem_read(sp+12,8)).hex()
     events.append(event)
   def record(uc,access,address,size,value,d):
    if pointer+0x1710<=address<pointer+0x1718 or address==pointer+0x1724:
     writes.append(dict(pc=hex(uc.reg_read(UC_X86_REG_EIP)),offset=hex(address-pointer),bytes=size,value=value))
   h=u.hook_add(UC_HOOK_CODE,observe);w=u.hook_add(UC_HOOK_MEM_WRITE,record)
   try:
    for begin,end,required in blocks:
     for reg,value in ((UC_X86_REG_ESP,SP),(UC_X86_REG_ESI,pointer),(UC_X86_REG_EDI,RULES)):u.reg_write(reg,value)
     run_checked(u,begin,end,required_addresses=[required])
   finally:u.hook_del(h);u.hook_del(w)
   row=dict(name=name,sections=sections,before=before,after=state(pointer),events=events,writes=writes,blocks=[[hex(a),hex(b)] for a,b,_ in blocks])
   if source is not None:row['source']=source
   return row
  guard=((0x670EBD,0x670EDD,0x474620),);idle=((0x66B3E4,0x66B40B,0x5283D0),)
  for name,sections in (('guard_missing',{}),('guard_wrong_case',{'General':{'guardmodestray':'2'}}),('guard_two',{'General':{'GuardModeStray':'2'}}),('guard_minus_one_sentinel',{'General':{'GuardModeStray':'-1'}}),('guard_negative_two',{'General':{'GuardModeStray':'-2'}}),('guard_later_missing_retains',{})):
   result['controls'].append(read(name,fresh,sections,guard))
  for name,sections in (('idle_missing',{}),('idle_wrong_case',{'AudioVisual':{'idleactionfrequency':'0.15'}}),('idle_retail_literal',{'AudioVisual':{'IdleActionFrequency':'0.15'}}),('idle_later_missing_retains',{})):
   result['controls'].append(read(name,fresh,sections,idle))
  result['handler_rules_before_physical']=state(self.rules)
  for name,path in native_owner.base.layers():
   if not path.exists():
    assert name=='LANGRULE.INI';result['physical_layers'].append(dict(file=name,absent=True));continue
   raw=path.read_bytes();sections,lines=lexical(raw,{'General','AudioVisual'})
   selected={section:{key:value for key,value in entries.items() if key==('GuardModeStray' if section=='General' else 'IdleActionFrequency')} for section,entries in sections.items()}
   row=read(name,self.rules,selected,guard+idle,dict(sha256=hashlib.sha256(raw).hexdigest(),lines=lines))
   result['physical_layers'].append(dict(file=name,**row))
  result['handler_rules_after_physical']=state(self.rules)
  self.phase='logic';self.events.clear();self.pending.clear()
  return result

 def registration(self):
  m,u=self.m,self.u
  arrays={}
  for name,header in (('techno',0xA8EC78),('unit',0x8B4108),('infantry',0xA83DE8),('logic',0x87F778),('ground_display',0x8A0390)):
   capacity=m.read32(header+8);count=m.read32(header+16);data=m.read32(header+4)
   assert count<=capacity<=1024
   arrays[name]=dict(header=hex(header),buffer=hex(data),capacity=capacity,count=count,actors=[hex(m.read32(data+i*4)) for i in range(count)])
  return arrays

 def initialize_greatest_candidates(self):
  m,u=self.m,self.u;self.pending.clear();self.events.clear();self.phase='setup'
  out=dict(before=self.registration(),rng_before={k:native_owner.base.sr.rng_state(u,p) for k,p in self.resident.rngs.items()},placements=[])
  for off in (0x5528,0x5578):
   u.mem_write(self.enemy+off+4,dwords(m.alloc(4096),1024));u.mem_write(self.enemy+off+0x10,dwords(0))
  donor=m.alloc(0x200);m.invoke(0x47BBF0,donor)
  owners=[m.read32(donor+off) for off in (0x54,0x58)];empty_cells=[]
  for xy,ptr in self.resident.ptrs.items():
   if m.read32(ptr+0xE4)==0 and m.read32(ptr+0xE8)==0:
    empty_cells.append(dict(cell=list(xy),before=[m.read32(ptr+off) for off in (0x54,0x58)],after=owners))
    for off,value in zip((0x54,0x58),owners):u.mem_write(ptr+off,dwords(value))
  out['native_empty_cell_defaults']=dict(constructor='0x0047BBF0',donor=hex(donor),owners=owners,cells=empty_cells)
  self.extra_candidates={}
  for label,typ,ctor,unlimbo,xy in (('E1',self.e1_type,0x517A50,0x51DFF0,(86,49)),('MTNK_tie',self.typ,0x7353C0,0x737BA0,(88,51))):
   p=m.alloc(0x1000);self.phase='setup';m.invoke(ctor,p,(typ,0));constructed=self.registration()
   u.mem_write(p+0x21C,dwords(self.enemy));u.mem_write(p+0x14C,dwords(self.enemy));self.phase='placement'
   attempts=[]
   for probe in (xy,(88,49),(87,51),(88,51),(86,51),(87,52),(88,52),(86,52),(87,49)):
    if probe not in self.resident.ptrs:continue
    m.invoke(0x486840,self.resident.ptrs[probe],(self.coord,));al=m.invoke(unlimbo,p,(self.coord,0x80))&255;attempts.append(dict(cell=list(probe),al=al))
    if al:xy=probe;break
   assert al,(label,attempts)
   if label=='E1':m.invoke(0x51D6F0,p,(0,0,0))
   self.extra_candidates[label]=p;self.prototype_baselines[p]=bytes(u.mem_read(p,0x1000))
   out['placements'].append(dict(label=label,actor=hex(p),constructor=hex(ctor),unlimbo=hex(unlimbo),cell=list(xy),attempts=attempts,al=al,xyz=native_owner.base.xyz(u,p+0x9C),display_layer=m.read32(p+0x94),logic_registered=u.mem_read(p+0x98,1)[0],flags=m.read32(p+0x14),limbo=u.mem_read(p+0x81,1)[0],after_constructor=constructed,after_unlimbo=self.registration()))
  self.cell_baselines={p:bytes(u.mem_read(p,0x200)) for p in self.resident.ptrs.values()}
  out.update(after=self.registration(),rng_after={k:native_owner.base.sr.rng_state(u,p) for k,p in self.resident.rngs.items()},events=list(self.events))
  self.phase='logic';self.events.clear();self.pending.clear()
  return out

def generate():
 q=FootMissions();q.initialize_companion();m,u=q.m,q.u
 rows=[]
 def add(name,family,**kw):
  row=q.row(f'{family}_{name}',family,**kw);rows.append(row)
 for family in ('MTNK','E1'):
  source=q.src if family=='MTNK' else q.e1
  xyz=native_owner.base.xyz(u,source+0x9C);ground=xyz[:];deck=[xyz[0],xyz[1],xyz[2]+416]
  for seed in (0,1,31):
   add(f'rescue_state2_seed{seed}',family,status=2,seed=seed)
   add(f'rescue_state1_seed{seed}',family,status=1,seed=seed)
  add('rescue_state1_nav_wait',family,status=1,nav=q.resident.ptrs[87,53])
  add('rescue_dispatch_state2',family,status=2,dispatch_entry=True)
  add('rescue_dispatch_state1',family,status=1,dispatch_entry=True)
  add('rescue_existing_target',family,status=0,target=q.resident.ptrs[87,51],moving=1,scan=1)
  add('area_guard_existing_target',family,mission=11,status=0,target=q.resident.ptrs[87,51],moving=1)
  add('area_guard_post',family,mission=11,status=0)
  add('area_guard_scan_empty',family,mission=11,status=0,scan_expired=True,idle_expired=True)
  add('area_guard_scan_live',family,mission=11,status=0,scan_expired=True,idle_expired=True,candidate_live=True)
  add('rescue_native_scan_live',family,status=0,archive=q.victim,stop=(RET_MAGIC,0x4DE12D),candidate_live=True,scan=1)
  add('rescue_native_scan_empty',family,status=0,archive=q.victim,stop=(RET_MAGIC,0x4DE12D),scan=1)
  add('rescue_default_ground_anchor',family,status=0,stop=(RET_MAGIC,0x4DE12D),candidate_live=True,scan=1)
  add('rescue_default_deck_anchor',family,status=0,stop=(RET_MAGIC,0x4DE12D),candidate_live=True,scan=1,
   source_xyz=deck,on_bridge=1,source_cell_flags=0x100,head_xyz=[23808,13184,832],nav=q.resident.ptrs[86,53])
  add('area_guard_default_deck_anchor',family,mission=11,status=0,scan_expired=True,idle_expired=True,
   source_xyz=deck,on_bridge=1,source_cell_flags=0x100,head_xyz=[23808,13184,832])
  add('rescue_entity_deck_anchor',family,status=0,archive=q.victim,archive_xyz=[23040,12928,832],
   stop=(RET_MAGIC,0x4DE12D),candidate_live=True,scan=1)
  add('rescue_zero_archive_point',family,status=0,archive=q.victim,archive_xyz=[0,0,0],
   stop=(RET_MAGIC,0x4DE12D),candidate_live=True,scan=1)
  add('rescue_nonzero_archive_point',family,status=0,archive=q.victim,archive_xyz=[0,0,1],
   stop=(RET_MAGIC,0x4DE12D),candidate_live=True,scan=1)
  edge=3840 if family=='MTNK' else 3072
  for delta in (edge-1,edge,edge+1):
   add(f'rescue_gate_{delta}',family,status=0,archive=q.victim,candidate_result=q.candidate,
    candidate_xyz=[23040+delta,12928,416],archive_xyz=[23040,12928,416],candidate_live=True,scan=1)
  for z in (416,832):
   add(f'rescue_gate_vertical_archive_z{z}',family,status=0,archive=q.victim,candidate_result=q.candidate,
    candidate_xyz=[23040+edge-20,12928,416],archive_xyz=[23040,12928,z],candidate_live=True,scan=1)
  add('rescue_gate_paid_head_irrelevant',family,status=0,archive=q.victim,candidate_result=q.candidate,
   candidate_xyz=[23040+edge-1,12928,416],archive_xyz=[23040,12928,416],candidate_live=True,scan=1,
   source_xyz=deck,on_bridge=1,source_cell_flags=0x100,head_xyz=[23808,13184,832],nav=q.resident.ptrs[86,53])
  leash=2816 if family=='MTNK' else 2252
  for delta in (leash-1,leash,leash+1,leash+2,leash+3):
   add(f'area_guard_cell_leash_{delta}',family,mission=11,status=0,archive=q.resident.ptrs[86,50],
    source_xyz=[22144-delta,12928,416])
  stray=native_owner.base.i32(u,q.rules+0x1724)
  for delta in (stray,stray+1,stray+2,stray+3):
   add(f'area_guard_foot_stray_{delta}',family,mission=11,status=0,archive=q.victim,
    archive_xyz=[ground[0]+delta,ground[1],ground[2]])
  add('area_guard_leash_firing_bypass',family,mission=11,status=0,archive=q.resident.ptrs[86,50],
   source_xyz=[22144-leash-3,12928,416],firing=1)
  add('area_guard_leash_nav_bypass',family,mission=11,status=0,archive=q.resident.ptrs[86,50],
   source_xyz=[22144-leash-3,12928,416],nav=q.resident.ptrs[87,53],moving=1)
  add('area_guard_foot_stray_clear_live_target',family,mission=11,status=0,archive=q.victim,archive_xyz=[ground[0]+stray+3,ground[1],ground[2]],target=q.candidate,candidate_live=True)
  add('area_guard_dispatch_post',family,mission=11,status=0,dispatch_entry=True)
 out=dict(schema_version=1,native_sha256=NATIVE_SHA256,text_sha256=q.resident.code_hash,inputs=q.inputs,world=q.world,
  mission_inputs=q.mission_inputs,e1_ctor=q.e1_ctor,e1_placement=q.e1_placement,e1_unlimbo=q.e1_unlimbo,
  candidate_placement=q.candidate_placement,candidate_unlimbo=q.candidate_unlimbo,victim_placement=q.victim_placement,victim_unlimbo=q.victim_unlimbo,initializers=q.initializers,
  setup=dict(source=hex(q.src),e1=hex(q.e1),victim=hex(q.victim),candidate=hex(q.candidate),house=hex(q.house),enemy=hex(q.enemy),country=hex(q.country),
   original_vtables={address:hashlib.sha256(bytes.fromhex(raw)).hexdigest() for address,raw in q.vtables.items()},
   physical_getcoords=[hex(a) for a in sorted(q.entry_getcoords)],stray=native_owner.base.i32(u,q.rules+0x1724),
   null_coord=native_owner.base.xyz(u,0xB0EA90),e1_nonvehicle=u.mem_read(q.e1_type+0x695,1)[0],
   elevation_cap_key=m.string(0x83B344),leash_key=m.string(0x83BD90)),rows=rows)

 out['initial_action_receipt']=q.initial_action_receipt
 out['rules_reader_receipts']=q.rules_reader_receipts();idle_rows=[]
 for seed in (0,1,31):
  for name,expired in (('area_guard_post',False),('area_guard_scan_empty',True)):
   idle_rows.append(q.row(f'E1_retail_idle_{name}_seed{seed}',family='E1',mission=11,status=0,seed=seed,scan_expired=expired,idle_expired=expired,idle_args=dict(entry='area_guard',doing=0)))
  idle_rows.append(q.row(f'E1_retail_idle_direct_seed{seed}',family='E1',mission=11,status=0,seed=seed,idle_expired=True,idle_args=dict(entry='idle',doing=0)))
 for doing in (1,16,-1,4,9,27,28,29,30):
  idle_rows.append(q.row(f'E1_retail_idle_admission_doing{doing}',family='E1',mission=11,status=0,idle_expired=True,idle_args=dict(entry='idle',doing=doing)))
 for name,control,kw in (('foot_flag_3d5',dict(entry='idle',doing=0),dict(moving=1)),('walk_moving',dict(entry='idle',doing=0,loco_moving=1),{}),('prone',dict(entry='idle',doing=0,prone=1),{}),('firing',dict(entry='idle',doing=0),dict(firing=1)),('future_timer',dict(entry='idle',doing=0,timer_words=[1,0,99]),{}),('target_nav_pose',dict(entry='idle',doing=0,frame_f8=7),dict(target=q.candidate,nav=q.resident.ptrs[87,53]))):
  idle_rows.append(q.row(f'E1_retail_idle_admission_{name}',family='E1',mission=11,status=0,idle_expired=True,idle_args=control,**kw))
 for seed in range(2,12):
  idle_rows.append(q.row(f'E1_retail_idle_direct_seed{seed}',family='E1',mission=11,status=0,seed=seed,idle_expired=True,idle_args=dict(entry='idle',doing=0)))
 for doing in (9,10):
  idle_rows.append(q.row(f'E1_retail_idle_completion_doing{doing}',family='E1',mission=11,status=0,idle_args=dict(entry='completion',doing=doing,loco_moving=0,prone=0)))
 out['retail_idle_rows']=idle_rows
 registration=q.initialize_greatest_candidates();greatest_rows=[]
 def scan(name,family,mask,latch,kind='MTNK',live=True,entry='concrete',point=(22144,12928,416),tie=False):
  result=q.row(f'{family}_{name}',family,status=0,archive=q.victim,archive_xyz=list(point),candidate_live=live and kind=='MTNK',scan=latch,
   greatest_args=dict(entry=entry,mask=mask,point_xyz=list(point),aux_bool=0,candidate_kind=kind,extra_live={'E1':live and kind=='E1','MTNK_tie':tie}))
  greatest_rows.append(result)
 for family in ('MTNK','E1'):
  for mask in (0,1,2):
   for latch in (0,1):
    for live in (False,True):
     scan(f'foot_literal_mask{mask}_latch{latch}_live{int(live)}',family,mask,latch,live=live,entry='foot')
    for kind in ('MTNK','E1'):
     for live in (False,True):
      scan(f'concrete_mask{mask}_latch{latch}_{kind}_live{int(live)}',family,mask,latch,kind=kind,live=live)
  for point_name,point in (('null_coord',(0,0,0)),('near_null_coord',(0,0,1)),('archive_deck',(23040,12928,832))):
   for latch in (0,1):
    scan(f'concrete_{point_name}_latch{latch}',family,0,latch,point=point)
  scan('concrete_global_equal_score_candidates',family,0,0,point=(22656,13056,416),tie=True)
 out['greatest_threat_registration']=registration;out['greatest_threat_rows']=greatest_rows
 # Additive downstream receipts run only after every historical row is captured.
 # Supplied limbo controls leave actual constructor/registration history intact.
 for ptr in q.extra_candidates.values():u.mem_write(ptr+0x81,bytes([1]))
 queue=m.alloc(8)
 control=dict(path=[2,3,4,5]+[-1]*20,reference_cell=[85,49],queue_backing=queue,
  queue_entries=[q.victim,q.resident.ptrs[87,53]],aux=q.candidate,
  movement_timer_words=[50,0,5],blocked_timer_words=[40,0,6],retry=7,blocked=1,
  inactive_registered_candidates=[q.candidate,*q.extra_candidates.values()])
 navigation_rows=[]
 for family in ('MTNK','E1'):
  source=q.src if family=='MTNK' else q.e1;xyz=native_owner.base.xyz(u,source+0x9C)
  row=q.row(f'{family}_area_guard_foot_stray_destination',family,mission=11,status=0,archive=q.victim,
   archive_xyz=[xyz[0]+515,xyz[1],xyz[2]],navigation_args=control)
  assert row['returned'] and any(e['kind']=='foot_destination' for e in row['events'])
  assert any(e['kind']=='drive_move_to' if family=='MTNK' else e['kind']=='walk_move_to' for e in row['events'])
  navigation_rows.append(row)
 out['navigation_rows']=navigation_rows
 home_rows=[]
 for family in ('MTNK','E1'):
  row=q.row(f'{family}_rescue_empty_home',family,mission=21,status=0,archive=q.victim,candidate_live=False,scan=1,
   navigation_args=control,home_args=dict(primary=[87,50],alternate=[0,0],radius=1024))
  assert row['returned'] and row['after']['status']==1 and row['after']['archive']=='0x0'
  assert all(any(e['kind']==kind for e in row['events']) for kind in ('house_return','house_random_return','random_direction','zone','nearby_cell'))
  home_rows.append(row)
 out['empty_rescue_rows']=home_rows
 # Complete every physically referenced E1 weapon only after all208 old rows.
 # The old unloaded Para/elite and omitted InvisibleLow state stays declared.
 out['weapon_reader_receipts']=q.weapon_reader_receipts();weapon_rows=[]
 retail_context=dict(reader_context='weapon_reader_receipts.after',rules_context='handler_rules_after_physical',registration_context='greatest_threat_registration.after')
 for delta in (2253,2254,2815,2816,2817,2818,2819):
  weapon_rows.append(q.row(f'E1_retail_weapons_area_guard_cell_leash_{delta}',family='E1',mission=11,status=0,
   archive=q.resident.ptrs[86,50],source_xyz=[22144-delta,12928,416],idle_args=dict(entry='area_guard',doing=0),evidence_context=retail_context))
 for delta in (3839,3840,3841):
  weapon_rows.append(q.row(f'E1_retail_weapons_rescue_gate_{delta}',family='E1',status=0,archive=q.victim,candidate_result=q.candidate,
   candidate_xyz=[23040+delta,12928,416],archive_xyz=[23040,12928,416],candidate_live=True,scan=1,evidence_context=retail_context))
 for live in (False,True):
  weapon_rows.append(q.row(f'E1_retail_weapons_rescue_native_scan_live{int(live)}',family='E1',status=0,archive=q.victim,
   stop=(RET_MAGIC,0x4DE12D),candidate_live=live,scan=1,evidence_context=retail_context,
   navigation_args=dict(control,inactive_registered_candidates=list(q.extra_candidates.values()))))
 out['retail_weapon_rows']=weapon_rows
 return out


NATIVE_SPANS = {
 'rescue':(0x4DDF90,0x23E),'area_guard':(0x4D6AA0,0x6C4),
 'unit_area_guard':(0x744100,0x75),'infantry_area_guard':(0x51F640,0x16),
 'mission_dispatch':(0x5B3060,0x486),'mission_control_reader':(0x5B3760,0x213),
 'elevation_reader':(0x66D150,0x98),'physical_object_coordinate':(0x5F65A0,0x25),
 'physical_cell_coordinate':(0x486840,0x47),'object_distance':(0x5F6360,0xD3),
 'fast_sqrt':(0x4CAC40,0x6E),'fast_sqrt_table':(0x8650BC,0x10000),
 'rescue_multiplier':(0x7E48F0,8),'area_guard_multiplier':(0x7E9258,8),
 'mission_rate_multiplier':(0x7E27F8,8),'null_coordinate_initializer':(0x6F2A50,0x20),
 'rules_constructor':(0x665650,0x23D7),'guard_mode_stray_key_block':(0x670EBD,0x20),
 'idle_action_frequency_key_block':(0x66B3E4,0x27),'ini_range_reader':(0x474620,0x3A),
 'ini_double_reader':(0x5283D0,0x1D2),'idle_action_frequency_literal':(0x83A338,20),
 'foot_greatest_threat':(0x4D9920,0x3F),'unit_greatest_threat':(0x743190,0xD4),
 'infantry_greatest_threat':(0x51E140,0x26C),'techno_greatest_threat':(0x6F8DF0,0xFBF),
 'techno_registry_append':(0x6F3183,0x52),'techno_global_fold':(0x6F9C67,0x148),
 'cell_constructor':(0x47BBF0,0x1B8),'logic_register':(0x55BAA0,0x36),
 'infantry_idle':(0x51CDB0,0x2F8),'infantry_idle_jump_table':(0x51D0A8,40),
 'infantry_idle_admission':(0x5216D0,0x86),'walk_is_moving':(0x75AB30,0xA),
 'infantry_action':(0x51D6F0,0x3F5),'primary_facing':(0x4C9300,0xCA),
 'game_options_delay':(0x5FB2E0,0x26),'infantry_sequencer':(0x520AE0,0x41C),
 'idle_max_multiplier':(0x7E44B8,8),'idle_min_multiplier':(0x7E5D78,8),
 'idle_random_unit_scale':(0x7E3570,8),
 'infantry_constructor':(0x517A50,0x269),'infantry_unlimbo':(0x51DFF0,0x144),
 'walk_constructor':(0x75AA90,0x67),'foot_destination':(0x4D94B0,0x264),
 'unit_destination':(0x741970,0x1817),'infantry_destination':(0x51AA40,0x7A1),
 'drive_move_to':(0x4AFD40,0xB5),'walk_move_to':(0x75ACB0,0xE1),
 'drive_is_moving':(0x4AFB80,0x96),'nav_queue_clear':(0x4E0190,0x2A),
 'house_ordinary_return':(0x500200,0xF9),'house_random_return':(0x501AC0,0x628),
 'house_base_center':(0x50DEF0,0x3C),'random_direction':(0x49F420,0x121),
 'clamp_map':(0x586E50,0x169),'nearby_cell':(0x56DC20,0xBA0),
 'zone':(0x56D230,0x16A),'raw_random':(0x65C780,0x51),'ranged_random':(0x65C7E0,0xAD),
 'weapon_constructor':(0x771C70,0x281),'weapon_find_or_allocate':(0x772FA0,0x83),
 'weapon_reader':(0x772080,0x965),'weapon_speed_postpass':(0x7729F0,0x5D),
 'projectile_constructor':(0x46BBC0,0x21E),'projectile_find_or_allocate':(0x46C790,0x83),'projectile_reader':(0x46BEE0,0x556),
 'warhead_constructor':(0x75CEC0,0x2F0),'warhead_reader':(0x75D3A0,0xB1E),
 'weapon_range':(0x7012C0,0xDB),'get_weapon':(0x70E140,0x56),
 'normal_weapon_slot':(0x7177C0,0x15),'elite_weapon_slot':(0x7177E0,0x15),
 'elite_predicate':(0x750010,0x18),'elite_threshold':(0x7E37B4,4),
}


def metadata():
 result=provenance(scope=__doc__,entry_points={
  'rules_constructor':0x665650,'guard_mode_stray_key_block':0x670EBD,
  'idle_action_frequency_key_block':0x66B3E4,'ini_range_reader':0x474620,'ini_double_reader':0x5283D0,
  'cell_constructor':0x47BBF0,'logic_register':0x55BAA0,'display_register':0x4A9720,
  'infantry_idle_admission':0x5216D0,'walk_is_moving':0x75AB30,
  'infantry_action':0x51D6F0,'primary_facing':0x4C9300,'game_options_delay':0x5FB2E0,'infantry_sequencer':0x520AE0,
  'mission_control_static_initializer':0x4E7CF0,'mission_control_reader':0x5B3760,
  'rules_mission_loop':0x679C92,'rules_elevation':0x66D150,'rules_general':0x66D530,
  'unit_constructor':0x7353C0,'infantry_type_constructor':0x5236A0,
  'infantry_type_reader':0x5240A0,'infantry_constructor':0x517A50,
  'drive_factory':0x6C4010,'walk_factory':0x6C4790,
  'unit_unlimbo':0x737BA0,'infantry_unlimbo':0x51DFF0,'infantry_ready':0x51D6F0,
  'rescue':0x4DDF90,'area_guard':0x4D6AA0,'mission_dispatch':0x5B3060,
  'unit_area_guard':0x744100,'infantry_area_guard':0x51F640,
  'archive_setter':0x70C610,'queue_mission':0x5B35E0,'commence':0x5B3570,
  'mission_control':0x5B3A00,'threat_range':0x707E60,
  'rescue_scan_call':0x4DE056,'rescue_refusal_stop':0x4DE12D,
  'unit_greatest_threat':0x743190,'infantry_greatest_threat':0x51E140,
  'foot_greatest_threat':0x4D9920,'greatest_threat':0x6F8DF0,
  'evaluate_target':0x6F7CA0,'threat_score':0x70CD10,
  'passive_scan':0x709820,'unit_approach':0x7414E0,'infantry_approach':0x522340,
  'foot_approach':0x4D5690,'object_distance':0x5F6360,'fast_sqrt':0x4CAC40,
  'unit_destination':0x741970,'infantry_destination':0x51AA40,'foot_destination':0x4D94B0,
  'assign_target':0x6FCDB0,'infantry_assign_target':0x51B1F0,
  'can_acquire':0x7091D0,'targeting_timer':0x70F7E0,'infantry_idle':0x51CDB0,
  'rng_seed':0x65C6D0,'rng_ranged':0x65C7E0,
  'walk_constructor':0x75AA90,'drive_is_moving':0x4AFB80,
  'drive_move_to':0x4AFD40,'walk_move_to':0x75ACB0,'nav_queue_clear':0x4E0190,
  'house_return':0x500200,'house_random_return':0x501AC0,'house_base_center':0x50DEF0,
  'random_direction':0x49F420,'clamp_map':0x586E50,'zone':0x56D230,'nearby_cell':0x56DC20,
  'weapon_constructor':0x771C70,'weapon_find_or_allocate':0x772FA0,'weapon_reader':0x772080,
  'projectile_constructor':0x46BBC0,'projectile_find_or_allocate':0x46C790,'projectile_reader':0x46BEE0,'warhead_constructor':0x75CEC0,'warhead_reader':0x75D3A0,
  'weapon_speed_postpass':0x7729F0,'weapon_range':0x7012C0,'get_weapon':0x70E140,
  'normal_weapon_slot':0x7177C0,'elite_weapon_slot':0x7177E0,'elite_predicate':0x750010,
 },assumptions=[
  'The additive weapon_reader_receipts runs only after all208 historical rows. Original normal7177C0/elite7177E0 slot getters expose M60/Para/M60E/ParaE names and actual retained constructor/read fields. Original7012C0 GetWeaponRange and707E60 ThreatRange run through original70E140 and native vtables for the retained rookie state and separately supplied veterancy float2.0 elite control. The getter control restores that supplied field and consumes no RNG. Before closure, only M60 has read physical WeaponType772080; its actual InvisibleLow46BEE0 attempt lacks a matching section and retains original46BBC0 constructor fields, while SA75D3A0 reads. The other three weapons retain original771C70 fields and null child references. Explicit omitted_sections/read attempts qualify the historical208 results; no old result is rewritten.',
  'New weapon closure reads exact-case physical E1-referenced normal/elite weapon sections and their Projectile/Warhead names across RULESMD, optional LANGRULE, mode and map layers. Every original772080, actual child46BEE0/75D3A0 and original7729F0 postpass executes, with native AL section status, reader arguments, observed fields/writes and full unchanged RNG recorded. No native range/default/scanner/child value comes from Rust. The separate12 retail_weapon_rows execute7 consecutive Cell-leash controls including the prior2254 contrast,3 strict Rescue supplied-result boundaries and2 actual scanner prefixes under the completed physical weapon context and physically read .15 idle frequency. Each new input names its weapon/rules/registration receipt context; actual ordered registries and every registered actor limbo/alive/coordinate/House are read before entry. The two real prefix controls reuse the existing navigation fixture to supply only the two extra candidates limbo1/alive0 after prototype restoration. Other new rows restore their actual placed extra baselines; the main candidate uses candidate_live. These continue original handlers, not a whole Rules Process or whole match.',
  'The additive initial_action_receipt captures original E1517A50/Walk75AA90 constructor state and successful51DFF0 Unlimbo before the companion explicit Ready. Both retain Doing-1. Full original520AE0 then executes actual COM Walk75AB30 and forced51D6F0(Ready0,1,0), recording the51D9D2 Doing write, raw frame/sequence/facing fields and full three RNG states. All probe writes and CPU/x87 are restored before explicit fixture Ready, preserving all204 historical values. The actor is the initialized stationary source at the declared crop/frame; first whole InfantryAI, moving admission, absolute-stage clock and whole ScenarioLoad are excluded.',
  'The additive two navigation_rows run original concrete AreaGuard through its Foot-anchor stray branch, original Unit741970/Infantry51AA40 destination setters, Foot4D94B0 and concrete Drive4AFD40/Walk75ACB0. Native+4C target-coordinate output, actual Map-returned Cell scalars, class setter entry/return RNG, destination/actual IsMoving/head, NavCom/Aux, all24 path words/reference, NavQueue and timers/sequence are recorded. Their nonempty path/queue/timer/Aux prestate is explicit supplied prior state, not producer evidence; native ctor locomotor head/destination are retained. Actual ILocomotion+10 getter is separately executed for before/after readback with its writes/CPU restored.',
  'The additive two empty_rescue_rows execute full original4DDF90 past4DE12D with actual empty scanner results, status1 store, House500200/501AC0/50DEF0,49F420, Map zone56D230 and FNPC56DC20, concrete Cell destination, archive clear and current-mission cadence. House retained physical fields+5490/+5494/+5498 are supplied primary87,50/alternate0,0/radius1024 under the existing house_base_projection.query_return field contract. Full three RNG states, ordered ranged/direction raw draws, original returned cells/coordinates, actual map queries and final retained Dummy state are captured; no House, FNPC, range, target or RNG result is replaced.',
  'Existing anytown_damage.mission.Mission is the sole physical source/crop/constructor bootstrap. Its original class and default native payload are unchanged. The extension adds original E1 type constructor5236A0/full layered reader5240A0, M60 and SA readers plus an explicitly unsuccessful InvisibleLow read attempt, and original Infantry517A50/Walk factory6C4790. Original Unit7353C0/Drive construction runs for the source, victim and enemy candidate; actual native Unlimbo, Mark, Display ground layer2 and Logic registration run for all four actors. Full physical E1 weapon closure is the separate additive receipt and rows.',
  'RULESMD.INI, absent LANGRULE.INI, MPBattleMD.ini and XMP03T4.MAP layers are explicit. MissionControl original static constructors/read loop, original66D150 ElevationModel and selected668BF0 threat coefficients execute per layer. Existing full66D530 supplies General.GuardModeStray. The preserved82 handler rows retain constructor IdleActionFrequency; they do not execute its AudioVisual key block. Fixed ART GI/GISequence is loaded with existing source-order cache; native E1 Ready51D6F0 executes before its baseline.',
  'Rows supply prior mission21 or11, status, queue-1, target/archive/NavCom, dispatch0/0, optional expired targeting/idle timer and explicit retained coordinate/layer/latch/head controls after original constructors/placement. Every row restores actor/locomotor/prototype/cell baseline; all native writes are journaled and rolled back after capture. Scenario65C6D0 reseeds with declared0/1/31 while Main/MapGen retained states remain explicit.',
  'Full original Rescue4DDF90, concrete Unit/Infantry AreaGuard wrappers and common4D6AA0 execute. Separate rows enter original dispatcher5B3060, including native timer epilogue. Actual RNG ranged/raw calls, stream identity, full three-stream states, physical+48 coordinates, mission control bits, original distance/fast-sqrt/ftol, target/destination/archive calls and selected field writes retain instruction order.',
  'Full-scanner rows execute original concrete+3C4 wrappers, Foot4D9920, Techno6F8DF0, evaluate6F7CA0 and score70CD10 on one native placed hostile MTNK. The unmodified real vtables and full text hashes are checked after each row. NullPoint0/0/0 comes from original6F2A50; zero and0/0/1 archive controls exercise actual native score branch and whole-cell versus leptonic distance.',
  'The additive rules_reader_receipts executes original665650 on explicit zero-initialized storage, records actual selected writes, and records the constructor retaining both supplied zero and0x12345678 at GuardModeStray+1724 without any overlapping native write. Its zero is a supplied storage control, not a native allocation/default claim. Original670EBD..670EDD/474620 and66B3E4..66B40B/5283D0 execute unchanged with supplied ESI Rules receiver and EDI existing CCINI cache; missing/exact-case/sentinel/negative/later-missing controls and the physical layers are explicit. The whole AudioVisual reader6691E0 is not run.',
  'The additive36 retail_idle_rows execute original E1 idle51CDB0, admission5216D0/7099E0 and actual ILocomotion+10 Walk75AB30, with selected Doing, actual interface+30 moving, prone, firing, timer, target/NavCom/animation-frame and seed controls. Original51D6F0 action,5FB2E0 delay and4C9300 body-facing execute. Two idle9/10 completion rows execute original action setup, supply stage at the real GI sequence count, then run full520AE0 through default520D1B and forced Ready; sequence table bytes and setup RNG are explicit. They do not validate the producer/absolute-stage clock. Idle timer/sequence/frame/facing bytes, full three RNG streams, instruction-ordered calls/writes and idle return RNG before AreaGuard cadence are recorded. Native FPCW is observed0x0E7F (the existing VM PC53/chop setting); native read .15 widens its parsedfloat bits before the original stored-f64 idle arithmetic.',
  'The additive86 greatest_threat_rows distinguish literal Foot4D9920 entry masks0/1/2 from original concrete+3C4 Unit743190/Infantry51E140 weapon-mask augmentation. Foot latch+688 rewrites only AL before original Techno6F8DF0 and clears only on an actual NULL return. Live MTNK/E1 candidates use original constructors/Unlimbo; no target or score return is supplied. Original array visits, evaluate/score outputs and strict better-score updates execute. Equal-score controls use a supplied midpoint and observe the earlier actual Techno registry candidate retained.',
  'greatest_threat_registration observes actual Techno/Unit/Infantry arrays after constructors, and Logic/ground Display arrays after original Unlimbo. Techno constructor appends; Logic appends for the supplied sort0 contract; Display layer2 uses its own ordering. Additional enemy E1 requires native empty Cell owner fields: original47BBF0 executes on a donor and only its+54/+58 values are supplied to empty cropped cells after all82 preserved rows. Full Cell/map construction is not claimed. Houses/counter vectors and declaring placed candidates limbo remain supplied boundaries.',
  'Cell anchors execute486840/47B3A0 on physical level/slope and remain ground anchors when the source pose is deck+416/OnBridge1. Entity anchors execute native5F65A0 physical XYZ. Drive/Walk retained-head offsets reuse locomotor_at_coord.FAMILIES; alternate head and unrelated NavCom are explicit controls. Full scan input, Rescue strict gate and AreaGuard leash use the archived physical point.',
 ],substitutions=[
  'Historical208 rows have an explicitly partial physical weapon read context: Para, M60E, ParaE, InvisibleLow and SSA sections are unread even though E1 TypeINI allocates all four slot weapon types. Their executed results and RNG remain unchanged. Old82 rows and initial_action_receipt use constructor IdleActionFrequency, while the later36 idle/86 threat/2 destination/2 home rows use the separately read physical frequency. The new weapon receipt publishes this array-to-reader-context map. New12 handler controls use all physically read E1 weapons and children but retain the inherited supplied crop, House, registration, archive/geometry and receiver-state bounds; strict Rescue result-seam rows do not certify actual acquisition.',
  'This is a selected handler/dispatcher continuation on the existing Anytown crop, not a native whole-match ScenarioLoad/RespondToBaseAttack/damage trigger or full live UnitAI/FootAI object tick. Queue promotion and source state that select these handlers are supplied. Full House construction/bookkeeping remain excluded; null-House object constructors are followed by explicit owner links and valid family-specific count vectors. Physical Americans country readers and original4F643B coefficient selection execute.',
  'Supplied-scanner-result rows execute the original Rescue prefix to4DE056, record actual virtual receiver/vtable/three arguments, then supply only the declared EAX result and consumed12-byte stack before running original caller4DE05C. They do not execute the scanner or certify candidate acquisition; full-scanner rows are separate. The preserved no-target/refused rows stop at4DE12D before status1 write, House500200 home selection, destination/archive cleanup or the cadence tail. The additive empty_rescue_rows execute that original suffix with the separate supplied House/crop premises.',
  'Geometry/layer/head/NullPoint and raw leash-boundary poses are supplied post-placement controls; they do not prove native bridge Unlimbo, locomotor head production, occupancy relocation or map loader chronology. Placed candidates can be declared limbo after actual placement to exercise empty scans; no physical UnInit transition is claimed. Crop and out-of-crop map/zone storage retain the existing owner bounds.',
  'Existing successful native heap/file/INI cache, empty SEH, wall-clock/atexit/ASCII/CLSID/Interlocked/OleRun OS boundaries remain. Additional Infantry COM activation is routed to original Walk factory after checking its physical CLSID/arguments. Visual asset/radar/sound sinks and inherited hierarchy586990 callback are recorded; full audio Main RNG/device work, rendering and future movement/occupancy effects are excluded. No range, RNG, approach, target, bridge or scanner result is replaced except the explicitly declared strict-gate scanner-result seam.',
  'New destination/home rows run after all204 historical rows are captured. The existing candidate and additional registered E1/MTNK tie candidates are explicitly supplied inactive with limbo+81=1 and alive+90=0 after row restores their original placement baselines. An earlier diagnostic set limbo before row; prototype restoration reset that candidate to limbo0/alive1 before native evaluation, explaining its selection. Native6F7D90/6F7D96/6F7D98 rejects a candidate whose limbo byte is nonzero, as the preserved greatest_threat_rows miss controls also demonstrate. Both bytes remain declared and recorded premises for the selected four downstream rows, and their original registration history remains real. House construction, base/radius producers, prior path/NavQueue/timer producers, full navigation initialization, movement Process and future occupancy/bridge relocation remain excluded; the inherited crop zone planes and resident runtime sinks apply to the full Rescue suffix.',
  'Raw EAX is recorded at callable returns; it is not a semantic return value for void/x87-returning methods. Native score_before_vhp and evaluate.score_result are integer score observations. The copied middle timer word is retained as raw stack data, not an authoritative timer field. These native vectors provide no Rust or production parity claim by themselves.',
 ])
 binary=image_bytes()
 result['native_spans']={}
 for name,(address,length) in NATIVE_SPANS.items():
  offset,raw=file_span(binary,address,length)
  row=dict(address=f'0x{address:08X}',file_offset=offset,length=length,sha256=hashlib.sha256(raw).hexdigest())
  if length<=0x700:row['hex']=raw.hex()
  result['native_spans'][name]=row
 return result


def source_paths():
 # Preserve the old historical receipt and use its transitive module inventory
 # as current source identity; never rewrite the historic source hashes.
 receipt=json.loads((HERE/'mission_receipt.json').read_text())
 paths={name:REPO/name for name in receipt['imported_sources']}
 for relative in ('tools/spatial_oracle/anytown_damage/foot_missions.py',
                  'tools/spatial_oracle/bridge_target_composed.py',
                  'tools/spatial_oracle/locomotor_at_coord.py',
                  'tools/spatial_oracle/house_base_projection.py',
                  'tools/native_slope.py',
                  'tools/spatial_oracle/fire_error.py'):
  paths[relative]=REPO/relative
 return paths


def publish(argv=None):
 finish_vectors(generate,HERE/'foot_missions.json',provenance=metadata,
                source_paths=source_paths(),argv=argv)
