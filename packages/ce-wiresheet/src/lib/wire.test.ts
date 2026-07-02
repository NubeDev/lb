import { describe, expect, it } from "vitest";
import { decodeBinaryFrame, type DecodedValue } from "./wire";
import {
  MSG_SNAPSHOT,
  MSG_UPDATE,
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

// --- A minimal frame encoder mirroring the layout decodeBinaryFrame expects.
// Keeping the encoder in the test (independent of the decoder) makes this a real
// round-trip / format-lock for the binary value plane.
const align8 = (n: number) => (n + 7) & ~7;
type Section = { typeTag: number; uids: number[]; write: (dv: DataView, at: number) => number };

function buildFrame(msgType: number, sections: Section[]): ArrayBuffer {
  const buf = new ArrayBuffer(8192);
  const dv = new DataView(buf);
  dv.setUint8(0, msgType);
  dv.setUint32(4, 7, true); // timestampMs
  dv.setUint8(8, sections.length);
  let off = 16;
  for (const s of sections) {
    off = align8(off);
    const start = off;
    dv.setUint8(start, s.typeTag);
    dv.setUint32(start + 4, s.uids.length, true);
    let p = start + 16;
    s.uids.forEach((u, i) => dv.setUint32(p + i * 4, u, true));
    p += s.uids.length * 4;
    if ((s.typeTag & 0xf0) === 0x20) p = align8(p); // 64-bit payload alignment
    const end = s.write(dv, p);
    dv.setUint32(start + 8, align8(end) - start, true); // sectionBytes
    off = start + (align8(end) - start);
  }
  return buf.slice(0, off);
}

const u32w = (v: number[]) => (dv: DataView, at: number) => {
  v.forEach((x, i) => dv.setUint32(at + i * 4, x, true));
  return at + v.length * 4;
};
const i32w = (v: number[]) => (dv: DataView, at: number) => {
  v.forEach((x, i) => dv.setInt32(at + i * 4, x, true));
  return at + v.length * 4;
};
const f32w = (v: number[]) => (dv: DataView, at: number) => {
  v.forEach((x, i) => dv.setFloat32(at + i * 4, x, true));
  return at + v.length * 4;
};
const f64w = (v: number[]) => (dv: DataView, at: number) => {
  v.forEach((x, i) => dv.setFloat64(at + i * 8, x, true));
  return at + v.length * 8;
};
const u64w = (v: bigint[]) => (dv: DataView, at: number) => {
  v.forEach((x, i) => dv.setBigUint64(at + i * 8, x, true));
  return at + v.length * 8;
};
const i64w = (v: bigint[]) => (dv: DataView, at: number) => {
  v.forEach((x, i) => dv.setBigInt64(at + i * 8, x, true));
  return at + v.length * 8;
};
const boolw = (v: boolean[]) => (dv: DataView, at: number) => {
  v.forEach((x, i) => dv.setUint8(at + i, x ? 1 : 0));
  return at + v.length;
};
const strw = (v: string[]) => (dv: DataView, at: number) => {
  const te = new TextEncoder();
  const blobs = v.map((s) => te.encode(s));
  const offs = [0];
  for (const b of blobs) offs.push(offs[offs.length - 1] + b.length);
  offs.forEach((o, i) => dv.setUint32(at + i * 4, o, true));
  let bo = at + offs.length * 4;
  const u8 = new Uint8Array(dv.buffer);
  for (const b of blobs) {
    u8.set(b, bo);
    bo += b.length;
  }
  return bo;
};

const vals = (buf: ArrayBuffer): DecodedValue[] => Array.from(decodeBinaryFrame(buf).sections[0].values);
const uids = (buf: ArrayBuffer): number[] => Array.from(decodeBinaryFrame(buf).sections[0].uids);

describe("decodeBinaryFrame", () => {
  it("reads the header (msgType, section count) and section uids", () => {
    const f = decodeBinaryFrame(buildFrame(MSG_SNAPSHOT, [{ typeTag: TYPE_U32, uids: [5, 6], write: u32w([1, 2]) }]));
    expect(f.msgType).toBe(MSG_SNAPSHOT);
    expect(f.sections).toHaveLength(1);
    expect(Array.from(f.sections[0].uids)).toEqual([5, 6]);
  });

  it("decodes BOOL", () => {
    const buf = buildFrame(MSG_UPDATE, [{ typeTag: TYPE_BOOL, uids: [1, 2, 3], write: boolw([true, false, true]) }]);
    expect(uids(buf)).toEqual([1, 2, 3]);
    expect(vals(buf)).toEqual([true, false, true]);
  });

  it("decodes U32 and I32", () => {
    expect(vals(buildFrame(MSG_UPDATE, [{ typeTag: TYPE_U32, uids: [1], write: u32w([4000000000]) }]))).toEqual([
      4000000000,
    ]);
    expect(vals(buildFrame(MSG_UPDATE, [{ typeTag: TYPE_I32, uids: [1], write: i32w([-5]) }]))).toEqual([-5]);
  });

  it("decodes F32 and F64", () => {
    expect(vals(buildFrame(MSG_UPDATE, [{ typeTag: TYPE_F32, uids: [1], write: f32w([1.5]) }]))[0]).toBeCloseTo(1.5);
    expect(vals(buildFrame(MSG_UPDATE, [{ typeTag: TYPE_F64, uids: [1], write: f64w([3.14159]) }]))[0]).toBeCloseTo(
      3.14159,
    );
  });

  it("decodes 64-bit ints: small → number, beyond MAX_SAFE → bigint", () => {
    const big = BigInt(Number.MAX_SAFE_INTEGER) + 10n;
    const u = vals(buildFrame(MSG_UPDATE, [{ typeTag: TYPE_U64, uids: [1, 2], write: u64w([42n, big]) }]));
    expect(u[0]).toBe(42);
    expect(u[1]).toBe(big);
    const i = vals(buildFrame(MSG_UPDATE, [{ typeTag: TYPE_I64, uids: [1], write: i64w([-9n]) }]));
    expect(i[0]).toBe(-9);
  });

  it("decodes variable-length strings (including empty)", () => {
    const buf = buildFrame(MSG_UPDATE, [{ typeTag: TYPE_STR, uids: [1, 2, 3], write: strw(["off", "auto", ""]) }]);
    expect(vals(buf)).toEqual(["off", "auto", ""]);
  });

  it("decodes multiple sections, keeping STATUS separate from values", () => {
    const f = decodeBinaryFrame(
      buildFrame(MSG_UPDATE, [
        { typeTag: TYPE_U32, uids: [10], write: u32w([99]) },
        { typeTag: TYPE_STATUS, uids: [10], write: u32w([0b100000]) },
      ]),
    );
    expect(f.sections).toHaveLength(2);
    expect(f.sections[0].typeTag).toBe(TYPE_U32);
    expect(Array.from(f.sections[0].values)).toEqual([99]);
    expect(f.sections[1].typeTag).toBe(TYPE_STATUS);
    expect(Array.from(f.sections[1].values)).toEqual([0b100000]);
  });
});
