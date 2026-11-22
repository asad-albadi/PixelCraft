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
 * playback=22(b=1: a=tag index; otherwise whole timeline; kind6 sequence/durations/repeat).
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
