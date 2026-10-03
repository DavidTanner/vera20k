"""Original ordinary tactical A production from stock SHROUD.SHP.

The existing rally fixture owns native Surface/ABuffer setup; stock.mix owns
archive extraction. Native fill, SHP access, edge selection, projection, clips
and shroud writes execute without gameplay substitutions. Fog/AlphaShapes are
explicitly absent; supplied cell flags are not a reveal-traversal oracle.
"""
from functools import lru_cache
import hashlib
from pathlib import Path
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESI, UC_X86_REG_ESP

from tools import native_oracle as native
from tools.bridge_click_oracle import MATRIX_INITIALIZER
from tools.procedural_drawing_oracle import rally
from tools.sidebar_oracle import stock
from tools.spatial_oracle.bridge_damage_admission import call, words, MEM, TABLE, SP, MAP

SHP, SCRATCH, CELL_BASE = MEM + 0xB0000, MEM + 0xD0000, 0x32000000
NEIGHBORS = ((-1,-1,64),(0,-1,128),(1,-1,1),(-1,0,32),(1,0,2),
             (-1,1,16),(0,1,8),(1,1,4))


def sha(data):
    return hashlib.sha256(data).hexdigest()


@lru_cache(maxsize=1)
def asset():
    outer = (native.configured_gamemd().parent / 'ra2.mix').read_bytes()
    inner = stock.mix(outer)[stock.mix_hash('conquer.mix')]
    raw = stock.mix(inner)[stock.mix_hash('shroud.shp')]
    return raw, dict(archive='ra2.mix/conquer.mix/SHROUD.SHP',
                    outer_sha256=sha(outer), inner_sha256=sha(inner), sha256=sha(raw))


class Shroud:
    def __init__(self, case=None):
        self.case = case or {}
        self.rally = rally.Rally(dict(camera=self.case.get('camera', [-320,440])))
        self.u = u = self.rally.u
        self.draws, self.cell_calls = [], []
        raw, _identity = asset()
        u.mem_write(SHP, raw)
        u.mem_write(0x89E7C5, b'\1')  # already-loaded SHROUD/FOG startup cache
        u.mem_write(0x89E794, words(SHP))
        u.mem_write(0xA8B230, words(SCRATCH))
        u.mem_write(SCRATCH, words(0))  # ordinary FogOfWar=no
        u.mem_write(0xB73550, words(1))  # active graphical-client gate
        u.mem_write(0x88A100, words(0))  # no AlphaShape objects
        u.mem_write(0xB0CE88, words(0))  # no additional dirty rectangles
        u.mem_map(CELL_BASE, 0x100000)
        u.mem_write(MAP + 0xF4, words(16,16))
        self.cells = {}
        for y in range(32):
            for x in range(32):
                p = CELL_BASE + (y*32+x)*0x200
                self.cells[x,y] = p
                u.mem_write(p, words(0x7E4EEC))
                u.mem_write(p+0x24, struct.pack('<hh',x,y))
                u.mem_write(p+0x120, b'\xfe\xfe')
                u.mem_write(p+0x130, words(1,0,0,0))
                u.mem_write(p+0x38, words(-1))
                u.mem_write(TABLE+(y*512+x)*4, words(p))
        u.reg_write(UC_X86_REG_ESP, SP)
        u.reg_write(UC_X86_REG_ESI, rally.TACTICAL)
        u.reg_write(UC_X86_REG_EBX, 0)
        native.run_checked(u, *MATRIX_INITIALIZER, count=100)
        self.scroll_rows = self.case.get('scroll_rows', 0)
        u.mem_write(rally.ABUFFER+0x10, words(self.scroll_rows*rally.SIZE[0]*2))
        u.mem_write(rally.ABUFFER+0x2C, words(rally.SIZE[1]))
        u.hook_add(UC_HOOK_CODE, self.observe)
        self.reset()

    def observe(self, u, pc, _size, _data):
        sp = u.reg_read(UC_X86_REG_ESP)
        if pc == 0x47EFE0:
            point, clip, frame = struct.unpack('<3I',u.mem_read(sp+4,12))
            self.draws.append(dict(point=rally.ints(u,point,2),
                                   clip=rally.ints(u,clip,4), frame=frame))
        elif pc == 0x4801F0:
            p = u.reg_read(UC_X86_REG_ECX)
            self.cell_calls.append(list(struct.unpack('<hh',u.mem_read(p+0x24,4))))

    def reset(self):
        call(self.u, 0x4112D0, rally.ABUFFER, (127,))
        assert self.alpha() == bytes([127]) * (rally.SIZE[0]*rally.SIZE[1])
        self.draws, self.cell_calls = [], []

    def alpha(self):
        data = [v[0] for v in struct.iter_unpack('<H',self.u.mem_read(
            rally.ALPHA,rally.SIZE[0]*rally.SIZE[1]*2))]
        assert max(data) <= 255
        offset = self.scroll_rows*rally.SIZE[0]
        return bytes(data[offset:]+data[:offset])

    def output(self):
        data = self.alpha()
        return dict(alpha_hex=data.hex(), alpha_sha256=sha(data),
                    draws=self.draws.copy(), cell_order=self.cell_calls.copy())

    def frame_rect(self, frame):
        call(self.u,0x69E7E0,SHP,(SCRATCH+0x180,frame))
        return rally.ints(self.u,SCRATCH+0x180,4)

    def leaf(self, case):
        self.reset()
        point, clip = case['point'], case.get('clip',[0,0,*rally.SIZE])
        self.u.mem_write(SCRATCH+0x100, words(*point))
        self.u.mem_write(SCRATCH+0x120, words(*clip))
        call(self.u,0x47EFE0,0,(SCRATCH+0x100,SCRATCH+0x120,case['frame']))
        return dict(input=case, frame_rect=self.frame_rect(case['frame']), **self.output())

    def set_flags(self, cell, flags):
        self.u.mem_write(self.cells[tuple(cell)]+0x12C,words(flags))

    def selector(self, mask, flags=0x18):
        center = (10,20)
        self.set_flags(center,flags)
        for dx,dy,bit in NEIGHBORS:
            self.set_flags((center[0]+dx,center[1]+dy),0 if mask&bit else 0x18)
        self.reset()
        self.u.mem_write(SCRATCH+0x100,words(40,30))
        self.u.mem_write(SCRATCH+0x120,words(0,0,*rally.SIZE))
        pointer = self.cells[center]
        call(self.u,0x4801F0,pointer,(SCRATCH+0x100,SCRATCH+0x120))
        return dict(flags=flags, mask=mask,
                    caches=list(struct.unpack('<bb',self.u.mem_read(pointer+0x120,2))),
                    frame=self.draws[0]['frame'], alpha_sha256=sha(self.alpha()))

    def scene(self, case):
        for xy in self.cells:
            self.set_flags(xy, case.get('default_flags',0))
        if (rect := case.get('revealed_rectangle')) is not None:
            for xy in self.cells:
                if rect[0]<=xy[0]<=rect[2] and rect[1]<=xy[1]<=rect[3]:
                    self.set_flags(xy,0x18)
        for override in case.get('cells',[]):
            self.set_flags(override['cell'],override['flags'])
            self.u.mem_write(self.cells[tuple(override['cell'])]+0x11B,
                             bytes([override.get('level',0)]))
        self.reset()
        dirty = case.get('dirty_cells',[[10,20]])
        self.u.mem_write(rally.TACTICAL+0xE0, words(len(dirty),*[self.cells[tuple(xy)] for xy in dirty]))
        self.u.mem_write(SCRATCH+0x100,words(0,0,0,0))
        self.u.mem_write(SCRATCH+0x120,words(0,0,*rally.SIZE))
        self.u.mem_write(SP,words(native.RET_MAGIC,SCRATCH+0x100,SCRATCH+0x100,
                                  SCRATCH+0x120,int(case.get('full_redraw',True))))
        self.u.reg_write(UC_X86_REG_ESP,SP)
        self.u.reg_write(UC_X86_REG_ECX,rally.TACTICAL)
        native.run_checked(self.u,0x6D3660,native.RET_MAGIC,count=2_000_000,
                           required_addresses=[0x6D3660,0x4801F0,0x47EFE0])
        native_output = self.output()
        # Replay the exact original per-cell calls in Rust's row order. No
        # Python pixel generator is used; original4801F0/47EFE0 perform stores.
        calls = list(zip(native_output['cell_order'],native_output['draws']))
        self.reset()
        for xy,draw in sorted(calls,key=lambda item:(item[0][1],item[0][0])):
            self.u.mem_write(SCRATCH+0x100,words(*draw['point']))
            self.u.mem_write(SCRATCH+0x120,words(0,0,*rally.SIZE))
            self.u.mem_write(SP,words(native.RET_MAGIC,SCRATCH+0x100,SCRATCH+0x120))
            self.u.reg_write(UC_X86_REG_ESP,SP)
            self.u.reg_write(UC_X86_REG_ECX,self.cells[tuple(xy)])
            native.run_checked(self.u,0x4801F0,native.RET_MAGIC,count=100_000)
        assert self.alpha().hex()==native_output['alpha_hex']
        result = dict(input=case, **native_output, row_major_full_plane_replay_equal=True)
        if case.get('rally'):
            self.rally.setup_building()
            self.rally.alpha = [v[0] for v in struct.iter_unpack('<H',self.u.mem_read(
                rally.ALPHA,rally.SIZE[0]*rally.SIZE[1]*2))]
            result['rally_passes'] = self.rally.draw_passes()
        return result


def stock_geometry():
    raw, identity = asset()
    w,h,frames = stock.shp(raw)
    masks=[]
    for frame in frames[:47]:
        masks.append({(frame['x']+i%frame['w'],frame['y']+i//frame['w'])
                      for i,v in enumerate(frame['pixels']) if v!=254})
    assert len({tuple(sorted(mask)) for mask in masks})==1
    overlaps=[]
    for dx,dy in ((30,15),(-30,15),(0,30),(60,0)):
        count=len(masks[0]&{(x+dx,y+dy) for x,y in masks[0]})
        assert count==0
        overlaps.append(dict(offset=[dx,dy],nontransparent_intersection=count))
    return dict(**identity,canvas=[w,h],frame_count=len(frames),
                active_frame_count=47,common_mask_pixels=len(masks[0]),
                frame15_values=sorted(set(frames[15]['pixels'])),
                active_values=sorted({v for f in frames[:47] for v in f['pixels']}),
                neighbor_mask_overlaps=overlaps)


def generate():
    f=Shroud()
    leaves=[f.leaf(dict(name=f'frame_{i}',frame=i,point=[40,30])) for i in range(47)]
    for frame in (0,15,33,46):
        for point in ([-60,-30],[-20,-10],[0,0],[130,100],[160,120]):
            leaves.append(f.leaf(dict(name=f'clip_{frame}_{point[0]}_{point[1]}',frame=frame,point=point)))
        leaves.append(f.leaf(dict(name=f'interior_clip_{frame}',frame=frame,
                                  point=[40,30],clip=[48,33,21,13])))
    for row in (1,119):
        wrapped=Shroud(dict(scroll_rows=row))
        leaves.append(wrapped.leaf(dict(name=f'circular_wrap_{row}',frame=15,
                                        point=[130,100],scroll_rows=row)))
    selectors=[f.selector(mask) for mask in range(256)]
    flag_controls=[f.selector(mask,flags) for flags in (0,0x08,0x10,0x18)
                   for mask in (0,1,0xAA,0xFF)]
    scene_inputs=[
        dict(name='full_clear',default_flags=0x18),
        dict(name='full_unrevealed',default_flags=0),
        dict(name='factory_frontier',revealed_rectangle=[0,0,12,31],rally=True),
        dict(name='island',revealed_rectangle=[9,18,12,21]),
        dict(name='frontier_camera_scroll',camera=[-331,437],revealed_rectangle=[0,0,12,31]),
        dict(name='frontier_circular_wrap',camera=[-289,451],scroll_rows=119,
             revealed_rectangle=[0,0,12,31]),
        dict(name='dirty_flat_cell',full_redraw=False,default_flags=0),
        dict(name='dirty_raised_cell_flat_A',full_redraw=False,default_flags=0,
             cells=[dict(cell=[10,20],flags=0,level=8)]),
        dict(name='dirty_partial_0x10_boundary',full_redraw=False,
             cells=[dict(cell=[10,20],flags=0x10)]),
    ]
    scenes=[Shroud(case).scene(case) for case in scene_inputs]
    assert scenes[-3]['alpha_hex']==scenes[-2]['alpha_hex']
    return dict(size=list(rally.SIZE),initial_value=127,world_y_bias=15,
                stock=stock_geometry(),leaf_cases=leaves,selector_cases=selectors,
                flag_controls=flag_controls,scenes=scenes)


if __name__=='__main__':
    native.finish_vectors(generate,Path(__file__).with_suffix('.json'),
        provenance=lambda:native.provenance(
            scope='Stock SHROUD ABuffer values; original4112D0 reset,47EFE0 blit,4801F0/6D8700 selectors and whole6D3660 ordinary dirty/full draw including6D71E0',
            assumptions=[
                'Retail ra2.mix/conquer.mix/SHROUD.SHP raw bytes and container hashes are recorded. Existing stock.mix decodes archives. No full native archive loader or mod/loose override resolver executes; production asset tests must establish the same SHA identity.',
                'rally.Rally owns original BSurface/ABuffer setup. Original4112D0 resets every fixture to127. Whole47EFE0 reads stock raw format0 frames through original69E7E0/69E740, clips, obtains circular rows4114B0 and stores bytes except254. Physical circular offset is supplied for two leaf controls and one full scene.',
                'Whole4801F0/6D8700 executes all256neighbor masks with center flags0x18;16separate flags0/0x08/0x10/0x18 controls record caches and frame choice. Cell flags/counters/table are supplied, not an executed reveal traversal. Partial0x10 controls establish the consumer boundary, not ordinary reachability.',
                'Whole6D3660 executes ordinary full-redraw6D71E0 or dirty-cell projection. Original matrix constructor stores6D1DC5..6D1E1E execute. Cell table32x32 and Size16x16 are supplied; reported scenes stay inside the ordinary map diamond. Native integer camera/clip points are saved; Rust world camera adds the existing15pixel Y bias. Fractional zoom is a separate presentation extension.',
                'Scenario FogOfWar flag isclear; active graphical-client gate is1; AlphaShape list and extra dirty-rectangle list areempty. Dynamic AlphaShapes, enabledFog, startup loader and incremental invalidation/scroll lifecycle are outside this common route.',
                'Row-order/full-plane equivalence replays exact native cell calls using original4801F0/47EFE0 in ry/rx order and a full-plane clip; no alternate Python rasterizer generates the reference. All47selected stock masks are disjoint on the flat lattice. Supplied factory rally fields reuse rally.Rally; one scene feeds actual original A output to6DA9D0/4C0750.',
            ], substitutions=['The optional visible rally surface inherits original BSurface storage/locking in place of DirectDraw from rally.Rally; original shroud ABuffer calls and gameplay leaves are not replaced.'],
            entry_points={'fill':0x4112D0,'shroud_blit':0x47EFE0,'cell_edges':0x4801F0,
                'selector':0x6D8700,'tactical':0x6D3660,'full_scan':0x6D71E0,
                'frame_rect':0x69E7E0,'frame_data':0x69E740,'circular_row':0x4114B0,
                'cell_center':0x480A30,'projection':0x6D1F10}),
        source_paths={'oracle':Path(__file__),'surface_fixture':Path(rally.__file__),
                      'stock_owner':Path(stock.__file__),
                      'projection_fixture':Path('tools/bridge_click_oracle.py')})
