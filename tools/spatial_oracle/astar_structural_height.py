"""Original AStar node-height prefix and selected-list fragment controls.

Execute 42A460..42A523 and 429E54..429E7F on supplied scalar Cells, node
descriptors and current heights. No CanEnter, full route or loader reachability
is claimed. Rust bridge_walkable is deliberately metadata only: native code
tests Cell+140 structural0x100 and signed ground bytes.
"""
from pathlib import Path
import hashlib,json,struct
from unicorn.x86_const import *
from tools.native_oracle import NATIVE_SHA256,RET_MAGIC,run_checked,provenance,finish_vectors
from tools.spatial_oracle.tube_hierarchy import initialized_process_fixture
from tools.spatial_oracle.map_queries import dwords,packed
from tools.spatial_oracle.guarded_arena import Arena


def execute(row):
    u,sp=initialized_process_fixture();arena=Arena(u)
    alloc=lambda n:arena.allocate(n,'supplied AStar scalar fragment input')
    pf=alloc(0xD00);node_pool=alloc(0x100004);descriptor_pool=alloc(0x180004)
    parent_cell=alloc(0x200);candidate_cell=alloc(0x200)
    parent_slot=alloc(4);candidate_slot=alloc(4);parent_descriptor=alloc(12);parent_node=alloc(16);goal=alloc(4)
    u.mem_write(pf+0x0C,dwords(descriptor_pool,node_pool));u.mem_write(pf+0x30,dwords(row['current_height']))
    for ptr,xy,key in ((parent_cell,[10,10],'parent'),(candidate_cell,[11,9],'candidate')):
        u.mem_write(ptr+0x24,packed(*xy));u.mem_write(ptr+0x11B,bytes([row[key+'_ground_raw']]))
        u.mem_write(ptr+0x140,dwords(row[key+'_flags']))
    u.mem_write(parent_slot,dwords(parent_cell));u.mem_write(candidate_slot,dwords(candidate_cell))
    u.mem_write(parent_descriptor,dwords(parent_slot,row['current_height'],0));u.mem_write(parent_node,dwords(parent_descriptor,0,0,1))
    u.mem_write(goal,packed(11,9))
    spans=((0x42A460,0x42A523),(0x429E54,0x429E7F))
    code=[bytes(u.mem_read(a,b-a))for a,b in spans]
    u.mem_write(sp,dwords(RET_MAGIC,0 if row.get('initial_node',False)else parent_node,candidate_slot,goal,0))
    u.reg_write(UC_X86_REG_ESP,sp);u.reg_write(UC_X86_REG_ECX,pf)
    run_checked(u,0x42A460,0x42A523,count=2000)
    descriptor=u.reg_read(UC_X86_REG_EDI)
    assert descriptor==descriptor_pool
    height=struct.unpack('<i',u.mem_read(descriptor+4,4))[0]
    counts=[struct.unpack('<I',u.mem_read(node_pool+0x100000,4))[0],struct.unpack('<I',u.mem_read(descriptor_pool+0x180000,4))[0]]
    assert counts==[1,1]
    u.mem_write(sp,bytes(0x80));u.mem_write(sp+0x60,b'\x7f')
    u.reg_write(UC_X86_REG_ESP,sp);u.reg_write(UC_X86_REG_ESI,pf);u.reg_write(UC_X86_REG_EBX,candidate_cell)
    run_checked(u,0x429E54,0x429E7F,count=2000)
    selected=bytes(u.mem_read(sp+0x60,1))[0];assert selected in (0,1)
    assert code==[bytes(u.mem_read(a,b-a))for a,b in spans]
    arena.guard_check()
    return dict(input=row,node_height=height,selected_list='ground'if selected else 'deck',
                selected_list_native_local=selected,pool_counters=counts,code_unchanged=True,allocation_guards_intact=True)

def allocation_guard(direction, allocated):
    u,sp=initialized_process_fixture();arena=Arena(u)
    slot=arena.allocate(4,'supplied native candidate table slot')
    cell=arena.allocate(0x200,'allocated candidate storage')
    u.mem_write(slot,dwords(cell if allocated else 0))
    u.mem_write(sp+0x18,dwords(direction))
    u.mem_write(0xABDC50+0x24,packed(71,77))
    dummy_before=bytes(u.mem_read(0xABDC50,0x200))
    code=bytes(u.mem_read(0x429E19,0x429E27-0x429E19))
    u.reg_write(UC_X86_REG_EAX,slot);u.reg_write(UC_X86_REG_ESP,sp)
    end=run_checked(u,0x429E19,(0x429E27,0x42A1A1),count=100,
                    required_addresses=(0x429E19,0x429E1F))
    assert code==bytes(u.mem_read(0x429E19,len(code)))
    arena.guard_check()
    return dict(direction=direction,allocated=allocated,skipped=end==0x42A1A1,
                endpoint=hex(end),dummy_unchanged=bytes(u.mem_read(0xABDC50,0x200))==dummy_before,
                selected_pointer_matches=u.reg_read(UC_X86_REG_EBX)==(cell if allocated else 0),
                original_span_sha256=hashlib.sha256(code).hexdigest())

def rows():
    result=[]
    def add(name,pg,pflags,current,cg,cflags,**extra):
        result.append(dict(name=name,parent_ground_raw=pg,parent_flags=pflags,current_height=current,
                           candidate_ground_raw=cg,candidate_flags=cflags,**extra))
    add('ordinary_ground_hills10',10,0,10,10,0)
    for cg in (5,6,7,8,9,10,11):add(f'structural_candidate_raw{cg}_from_ground10',10,0,10,cg,0x100)
    add('candidate_walkable_only_raw6_from_ground10',10,0,10,6,0,candidate_rust_bridge_walkable=True)
    add('candidate_not_walkable_raw6_from_ground10',10,0,10,6,0,candidate_rust_bridge_walkable=False)
    add('structural_parent_on_deck_continues_structural_candidate',6,0x100,10,2,0x100)
    add('walkable_only_parent_does_not_force_deck_continuation',6,0,10,2,0x100,parent_rust_bridge_walkable=True)
    add('structural_parent_under_deck_keeps_candidate_ground',6,0x100,6,3,0x100)
    add('walkable_only_parent_allows_ground_to_deck_transition',6,0,6,3,0x100,parent_rust_bridge_walkable=True)
    add('structural_parent_deck_to_candidate_walkable_only',6,0x100,10,6,0,candidate_rust_bridge_walkable=True)
    add('raw127_structural_deck_is131',127,0x100,131,127,0x100)
    add('raw127_walkable_only_candidate_stays127',127,0x100,131,127,0,candidate_rust_bridge_walkable=True)
    add('raw128_structural_deck_is_minus124',128,0x100,-124,128,0x100)
    add('raw255_ground_to_raw252_structural_deck_is0',255,0,-1,252,0x100)
    add('signed_minus128_to127_is_large_selected_deck',128,0,-128,127,0x100)
    add('signed_minus1_to0_is_adjacent_selected_ground',255,0,-1,0,0x100)
    add('initial_node_preserves_supplied131',127,0,131,127,0,initial_node=True)
    return result

def generate():
    result=dict(schema_version=1,native_sha256=NATIVE_SHA256,scope=__doc__,cases=[execute(row)for row in rows()],
                allocation_guards=[allocation_guard(direction,allocated) for direction in (2,8) for allocated in (False,True)],
                harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    return result

def metadata():
    return provenance(scope=__doc__,entry_points={'node_height_prefix_begin':0x42A460,'node_height_prefix_stop':0x42A523,'selected_list_begin':0x429E54,'selected_list_stop':0x429E7F,'candidate_pointer_guard':0x429E19},
                    assumptions=['Supplied Cells and parent node/descriptor data. Arena provides distinct preallocated original-size node/descriptor pools; original first allocation counters and descriptor writes execute.',
                                 'Node prefix stops before g/heuristic computation and node completion; selected-list fragment stops before hierarchy lookup. No concrete entry, closed-list admission, route reconstruction or full-search comparison.',
                                 'Raw ground bytes cover native storage-domain sign extension and deck+4. These controls do not claim map-loader or legal gameplay reachability for all values.',
                                 'Rust bridge_walkable hints are labels only and never appear in the native image. Structural Cell+140 bit0x100 alone changes the original branches.',
                                 'Candidate allocation controls supply a null or real table slot for compass2/tube8 and execute429E19 through the skip or allocated continuation. They stop before any Cell layer/zone/entry semantics and compare retained Dummy bytes unchanged.'],substitutions=[])

if __name__ == "__main__":
    import sys
    if '--hills-markers' in sys.argv:
        from tools.spatial_oracle import astar_hills_route, bridge_records
        hills = astar_hills_route.inputs
        finish_vectors(astar_hills_route.generate_markers, Path(__file__).with_name('astar_hills_markers.json'),
                       provenance=astar_hills_route.marker_metadata,
                       argv=[arg for arg in sys.argv[1:] if arg != '--hills-markers'],
                       source_paths={'height_owner':Path(__file__), 'hills_route':Path(astar_hills_route.__file__),
                                     'hills_inputs':Path(hills.__file__), 'mtnk_inputs':Path(hills.astar_mtnk_inputs.__file__),
                                     'navigation_inputs':Path(hills.navigation_inputs.__file__),
                                     'navigation':Path(hills.navigation.__file__),
                                     'bridge_constructor':Path(hills.bridge_constructor.__file__),
                                     'bridge_records':Path(bridge_records.__file__)})
    elif '--hills-route' in sys.argv:
        from tools.spatial_oracle import astar_hills_route, bridge_records
        hills = astar_hills_route.inputs
        finish_vectors(astar_hills_route.generate, Path(__file__).with_name('astar_hills_route.json'),
                       provenance=astar_hills_route.metadata,
                       argv=[arg for arg in sys.argv[1:] if arg != '--hills-route'],
                       source_paths={'height_owner':Path(__file__), 'hills_route':Path(astar_hills_route.__file__),
                                     'hills_inputs':Path(hills.__file__), 'mtnk_inputs':Path(hills.astar_mtnk_inputs.__file__),
                                     'navigation_inputs':Path(hills.navigation_inputs.__file__),
                                     'navigation':Path(hills.navigation.__file__),
                                     'bridge_constructor':Path(hills.bridge_constructor.__file__),
                                     'bridge_records':Path(bridge_records.__file__)})
    elif '--hills-inputs' in sys.argv:
        from tools.spatial_oracle import astar_hills_bridge_inputs
        finish_vectors(astar_hills_bridge_inputs.generate, Path(__file__).with_name('astar_hills_bridge_inputs.json'),
                       provenance=astar_hills_bridge_inputs.metadata,
                       argv=[arg for arg in sys.argv[1:] if arg != '--hills-inputs'],
                       source_paths={'height_owner':Path(__file__), 'hills_inputs':Path(astar_hills_bridge_inputs.__file__),
                                     'mtnk_inputs':Path(astar_hills_bridge_inputs.astar_mtnk_inputs.__file__),
                                     'navigation_inputs':Path(astar_hills_bridge_inputs.navigation_inputs.__file__),
                                     'navigation':Path(astar_hills_bridge_inputs.navigation.__file__),
                                     'bridge_constructor':Path(astar_hills_bridge_inputs.bridge_constructor.__file__)})
    elif '--threat-inputs' in sys.argv:
        from tools.spatial_oracle import astar_threat_inputs, astar_mtnk_inputs
        finish_vectors(astar_threat_inputs.generate, Path(__file__).with_name('astar_threat_inputs.json'),
                       provenance=astar_threat_inputs.metadata,
                       argv=[arg for arg in sys.argv[1:] if arg != '--threat-inputs'],
                       source_paths={'height_owner':Path(__file__), 'threat_inputs':Path(astar_threat_inputs.__file__),
                                     'mtnk_inputs':Path(astar_mtnk_inputs.__file__),
                                     'navigation_inputs':Path(astar_threat_inputs.navigation_inputs.__file__)})
    elif '--mtnk-inputs' in sys.argv:
        from tools.spatial_oracle import astar_mtnk_inputs
        finish_vectors(astar_mtnk_inputs.generate, Path(__file__).with_name('astar_mtnk_inputs.json'),
                       provenance=astar_mtnk_inputs.metadata,
                       argv=[arg for arg in sys.argv[1:] if arg != '--mtnk-inputs'],
                       source_paths={'height_owner':Path(__file__), 'mtnk_inputs':Path(astar_mtnk_inputs.__file__),
                                     'navigation_inputs':Path(astar_mtnk_inputs.navigation_inputs.__file__),
                                     'next_family':Path(astar_mtnk_inputs.next_family_native.__file__)})
    elif '--finishing' in sys.argv:
        from tools.spatial_oracle import astar_path_finishing
        finish_vectors(astar_path_finishing.generate, Path(__file__).with_name('astar_path_finishing.json'),
                       provenance=astar_path_finishing.metadata,
                       argv=[arg for arg in sys.argv[1:] if arg != '--finishing'],
                       source_paths={'height_owner':Path(__file__),'finishing':Path(astar_path_finishing.__file__)})
    else:
        finish_vectors(generate, Path(__file__).with_suffix(".json"), provenance=metadata)
