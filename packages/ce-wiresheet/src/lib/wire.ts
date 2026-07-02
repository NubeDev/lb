import {
  TYPE_BOOL,
  TYPE_F32,
  TYPE_F64,
  TYPE_I32,
  TYPE_I64,
  TYPE_STATUS,
  TYPE_STR,
  TYPE_U32,
  TYPE_U64,
} from "./engine-types";

// Binary frame decoder for the ce-rest WS protocol.
// Frame layout (little-endian throughout):
//   header (16 bytes):
//     u8  msgType   (0x01 update / 0x02 snapshot)
//     3   ─         reserved
//     u32 timestampMs
//     u8  sectionCount
//     7   ─         reserved
//   each section (8-byte aligned start):
//     u8  typeTag
//     3   ─         reserved
//     u32 count N
//     u32 sectionBytes   (forward-compat: skip unknown sections by adding this)
//     4   ─         reserved
//     u32[N] uids
//     ..  pad      (align to 8 for 64-bit payloads)
//     ..  payload  (per typeTag)
//     ..  pad to next 8-byte boundary

const alignUp = (n: number, m: number) => (n + (m - 1)) & ~(m - 1);

// Module-level decoder reused across frames — `new TextDecoder()` per STR
// section per frame is needless allocation on the decode hot path.
const STR_DECODER = new TextDecoder();

export type DecodedValue = number | bigint | boolean | string;

export interface DecodedSection {
  typeTag: number;
  uids: Uint32Array;
  values: DecodedValue[] | Float64Array | Float32Array | Int32Array | Uint32Array;
}

export interface DecodedFrame {
  msgType: number;
  timestampMs: number;
  sections: DecodedSection[];
}

export function decodeBinaryFrame(buf: ArrayBuffer): DecodedFrame {
  const view = new DataView(buf);
  const msgType = view.getUint8(0);
  const timestampMs = view.getUint32(4, true);
  const sectionCount = view.getUint8(8);
  const sections: DecodedSection[] = [];
  let off = 16;
  for (let s = 0; s < sectionCount; s++) {
    off = alignUp(off, 8);
    const typeTag = view.getUint8(off);
    const count = view.getUint32(off + 4, true);
    const sectionBytes = view.getUint32(off + 8, true);
    const sectionStart = off;
    const uidsOff = off + 16;
    const uids = new Uint32Array(buf, uidsOff, count);
    let payloadOff = uidsOff + count * 4;
    const is64 = (typeTag & 0xf0) === 0x20;
    if (is64) payloadOff = alignUp(payloadOff, 8);

    let values: DecodedSection["values"];
    switch (typeTag) {
      case TYPE_BOOL: {
        const bytes = new Uint8Array(buf, payloadOff, count);
        const out = new Array<boolean>(count);
        for (let i = 0; i < count; i++) out[i] = bytes[i] !== 0;
        values = out;
        break;
      }
      case TYPE_U32:
      case TYPE_STATUS:
        // STATUS uses the same 4-byte uint32 payload as U32; downstream routes
        // STATUS sections to the per-uid statusFlags map instead of the value
        // map by checking the typeTag.
        values = new Uint32Array(buf, payloadOff, count);
        break;
      case TYPE_I32:
        values = new Int32Array(buf, payloadOff, count);
        break;
      case TYPE_F32:
        values = new Float32Array(buf, payloadOff, count);
        break;
      case TYPE_U64: {
        const u = new BigUint64Array(buf, payloadOff, count);
        // Convert to number for UI display when within safe range; otherwise keep
        // bigint so the caller can decide.
        const out = new Array<number | bigint>(count);
        for (let i = 0; i < count; i++) {
          const v = u[i];
          out[i] = v <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(v) : v;
        }
        values = out as DecodedValue[];
        break;
      }
      case TYPE_I64: {
        const a = new BigInt64Array(buf, payloadOff, count);
        const out = new Array<number | bigint>(count);
        for (let i = 0; i < count; i++) {
          const v = a[i];
          out[i] =
            v <= BigInt(Number.MAX_SAFE_INTEGER) && v >= BigInt(Number.MIN_SAFE_INTEGER)
              ? Number(v)
              : v;
        }
        values = out as DecodedValue[];
        break;
      }
      case TYPE_F64:
        values = new Float64Array(buf, payloadOff, count);
        break;
      case TYPE_STR: {
        const offsets = new Uint32Array(buf, payloadOff, count + 1);
        const blobOff = payloadOff + (count + 1) * 4;
        const blob = new Uint8Array(buf, blobOff, offsets[count]);
        const dec = STR_DECODER;
        const out = new Array<string>(count);
        for (let i = 0; i < count; i++) {
          out[i] = dec.decode(blob.subarray(offsets[i], offsets[i + 1]));
        }
        values = out;
        break;
      }
      default:
        // Unknown typeTag — skip cleanly using sectionBytes.
        values = [];
        break;
    }
    sections.push({ typeTag, uids, values });
    off = sectionStart + sectionBytes;
  }
  return { msgType, timestampMs, sections };
}
