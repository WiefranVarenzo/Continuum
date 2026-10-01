import { describe, expect, it } from "vitest";
import { AudioPcmQueue } from "./audioPcmQueue";

describe("system audio render-thread queue", () => {
  it("keeps stereo channels in order across wraparound", () => {
    const queue = new AudioPcmQueue(4, 1);
    queue.push(Int16Array.from([8192, -8192, 16384, -16384, 24576, -24576]));
    const left = new Float32Array(2), right = new Float32Array(2);
    queue.render(left, right);
    expect([...left]).toEqual([0.25, 0.5]); expect([...right]).toEqual([-0.25, -0.5]);
    queue.push(Int16Array.from([4096, -4096, 2048, -2048]));
    queue.render(left, right);
    expect([...left]).toEqual([0.75, 0.125]); expect([...right]).toEqual([-0.75, -0.125]);
  });
  it("bounds a delayed burst and retains its latest audio instead of accumulating latency", () => {
    const queue = new AudioPcmQueue(4, 1);
    queue.push(Int16Array.from([1, -1, 2, -2, 3, -3]));
    queue.push(Int16Array.from([4, -4, 5, -5, 6, -6]));
    expect(queue.queuedFrames).toBe(4);
    const left = new Float32Array(4), right = new Float32Array(4);
    queue.render(left, right);
    expect([...left].map(value => value * 32768)).toEqual([3, 4, 5, 6]);
    expect([...right].map(value => value * 32768)).toEqual([-3, -4, -5, -6]);
  });
  it("outputs silence through an underrun and resumes after a short rebuffer", () => {
    const queue = new AudioPcmQueue(8, 2);
    const left = new Float32Array(4), right = new Float32Array(4);
    queue.render(left, right);
    expect([...left]).toEqual([0, 0, 0, 0]);
    queue.push(Int16Array.from([16384, -16384, 8192, -8192]));
    queue.render(left, right);
    expect([...left]).toEqual([0.5, 0.25, 0, 0]);
    queue.render(left, right); expect([...left]).toEqual([0, 0, 0, 0]);
    queue.push(Int16Array.from([4096, -4096]));
    queue.render(left, right); expect([...left]).toEqual([0, 0, 0, 0]);
    queue.push(Int16Array.from([2048, -2048]));
    queue.render(left, right); expect([...left]).toEqual([0.125, 0.0625, 0, 0]);
  });
  it("survives a minute of packets with repeated IPC stalls and bursts", () => {
    const queue = new AudioPcmQueue();
    const packet = new Int16Array(4800).fill(8192);
    const left = new Float32Array(128), right = new Float32Array(128);
    let audible = 0;
    for (let tick = 0; tick < 1200; tick++) {
      if (tick % 40 >= 6) queue.push(packet);
      if (tick % 40 === 6) for (let delayed = 0; delayed < 6; delayed++) queue.push(packet);
      for (let quantum = 0; quantum < 19; quantum++) {
        queue.render(left, right); if (left.some(sample => sample !== 0)) audible++;
      }
      expect(queue.queuedFrames).toBeLessThanOrEqual(12000);
    }
    expect(audible).toBeGreaterThan(18000);
  });
});
