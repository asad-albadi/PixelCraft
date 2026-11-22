#ifndef PIXELCRAFT_FFI_H
#define PIXELCRAFT_FFI_H
#include <stddef.h>
#include <stdint.h>
/* ABI 1. Serialize calls and buffer access on one owning worker.
 * Handles are opaque and never reused. Zero means allocation/fatal failure.
 * Borrowed pointers remain valid only until buffer_free; never free them with
 * another allocator. Write only to caller-owned input buffers (kind 4), before
 * submitting them. Copy returned bytes before freeing the buffer. Never retain
 * a view across free or access/free it concurrently. No Rust layout is exposed.
 * Kinds: error UTF-8=0, state JSON=1, RGBA8=2, native .pixel=3, input=4, private-recovery=5, typed JSON=6.
 * Limits: 2 sessions, 64 buffers, 8 MiB each / 32 MiB total, canvas <=256x256.
 */
uint32_t pc_abi_version(void);
uint64_t pc_buffer_alloc(size_t length);
const uint8_t *pc_buffer_data(uint64_t buffer);
size_t pc_buffer_len(uint64_t buffer);
uint32_t pc_buffer_kind(uint64_t buffer);
uint32_t pc_buffer_free(uint64_t buffer);
uint32_t pc_session_close(uint64_t session);
/* new=0(a=width,b=height),demo=1,state=2,undo=3,redo=4,encode=5,
 * decode=6(input native bytes),RGBA=7,frame=9(a=index),layer=10(a=index),
 * saved=12(a=encoded revision, acknowledge only AFTER durable file publication),
 * recovery snapshot=15(kind5), restore=16(input snapshot), commands=17(input JSON:
 * {revision,commands:[...]}, <=32 commands/64 KiB; mobile allowlist only),
 * brush=18(a=0 round,1 square,2 diamond,3 horizontal,4 vertical; not an edit),
 * sample=19(a=x,b=y;kind6 {color:[r,g,b,a]}), selection=20(input JSON revision/action),
 * onion=21(a=previous,b=next; <=8 each; RGBA canvas with shared onion compositing),
 * playback=22(b=1: a=tag index; otherwise whole timeline; kind6 sequence/durations/repeat),
 * import=23(a=1 PNG/APNG,2 GIF,3 ASE; raw input; existing session required).
 * Import atomically replaces like decode6, resetting history/selection/dirty baseline.
 * export=24(a=1 active-frame PNG,2 all-frame GIF,3 ASE,4 sheet PNG,5 sheet JSON;
 * raw output kind3). Sheets pair as PixelCraft-sheet.png/PixelCraft-sheet.json: eight columns, no padding,
 * shared core names/durations/tags. Neither operation opens arbitrary paths.
 * Format limits: input/output <=8 MiB, side <=256, <=64 frames, <=32 layers,
 * <=16 MiB raster. Conservative ASE preflights can reject otherwise valid files.
 * PNG preserves indexed slots/APNG when supported; ancillary PNG metadata omitted.
 * GIF imports flattened RGBA and exports palette-quantized colors/10ms timing;
 * GIF cannot retain editable layers, tags, names or exact source palette indices.
 * ASE preserves supported editable metadata/indices (native frame names are omitted;
 * sheet JSON retains them). Mobile tilemaps/tilesets are
 * rejected before decode (including external references), as are off-canvas cels.
 * PNG export is static.
 * advanced=25(input JSON {revision,action,...}; one atomic Undo entry):
 * paint-tile(x,y CANVAS PIXEL coordinates,tile=0..count,optional flip_x/flip_y/
 * flip_diagonal booleans) resolves shared tile geometry and honors layer locks.
 * create-tileset(tile_width,tile_height 1..64,optional hide_source=false) uses the
 * active composite and shared conversion; <=256 source grid cells, <=8 tilesets.
 * Adds/selects a populated tile layer; tile0 is implicit empty,1..count are images.
 * hide_source=true hides existing layers in the same Undo step, retaining originals.
 * reference-set(index,x,y,scale,opacity,visible,optional scale_y): absent scale_y
 * preserves nonuniform height; null clears to uniform; number must be .01..16.
 * x/y finite within +/-4096,
 * scale .01..16,opacity0..255; reference-remove(index),reference-to-layer(index).
 * Locked references cannot be changed,removed,or converted.
 * reference-import=26(a=revision,input raw PNG/APNG; first composited frame only),
 * appends an embedded reference without source paths; at most 4 references/8 tilesets.
 * Native opens with linked reference source paths are rejected; embed references first.
 * reference-preview=27(a=reference index; raw intrinsic RGBA kind2; dimensions in state).
 * State includes references index/name/width/height/x/y/scale/scale_y/behind/floating/
 * opacity/visible/locked,
 * tilesets index/name/width/height/count, and each layer's nullable tileset index.
 * prepare-import=28(a=0 native .pixel,1 PNG/APNG,2 GIF,3 ASE; raw input),
 * validates one candidate per session without editing active artwork/history/target;
 * kind6 {prepared:true}. A second prepare fails until canceled or committed.
 * commit-import=29(a=revision observed before prepare) only commits if stored and
 * current revisions match a; same replacement/save/history reset as decode6.
 * cancel-import=30 drops candidate without editing active artwork/history/target.
 * Closing a session frees its candidate. Parse runs to bounded completion; caller
 * cancellation uses op30 after prepare returns. Other edits make commit stale.
 * stroke-size=31(a=1..32; requires active stroke; allowed during stroke) changes
 * size for subsequent stroke segments only; no revision/history entry by itself.
 * Finish records one Undo entry for all segments; cancel restores the whole stroke.
 * RGBA op7 b=1 reads frame a without changing the edit target; b=0 reads active frame.
 * session=0 creates an editor with new/demo/decode; other unknown IDs fail.
 */
uint64_t pc_request(uint64_t session, uint32_t operation, uint32_t a,
                    uint32_t b, uint64_t input);
/* begin=0,extend=1,finish=2,cancel=3. Input <=256 little-endian i32 (x,y) pairs.
 * color packs RGBA bytes little-endian; size=1..32 on begin. Empty points are
 * allowed on finish/cancel. Returns 0 success, -1 closed, -2 buffer, -3 operation,
 * -99 contained panic. A failed batch does not authorize further unchecked use.
 */
int32_t pc_stroke(uint64_t session, uint32_t phase, uint64_t input,
                  uint32_t color, uint32_t size);
#endif

/* Recovery: op15 returns private snapshot kind5; op16 restores an input snapshot.
 * Restores the unsaved baseline and frame/layer, not history. Not a .pixel file. */
