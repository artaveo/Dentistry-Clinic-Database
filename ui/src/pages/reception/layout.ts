/** Side-by-side lanes for appointments that overlap in one column of the calendar. */
export type Box = { id: string; start: number; end: number };
export type Placed = Box & { lane: number; lanes: number };

export function layoutLanes(boxes: Box[]): Placed[] {
  const sorted = [...boxes].sort((a, b) => a.start - b.start || a.end - b.end || a.id.localeCompare(b.id));
  const out: Placed[] = [];
  let cluster: Placed[] = [];
  let clusterEnd = -1;
  const flush = () => {
    const lanes = cluster.reduce((m, p) => Math.max(m, p.lane + 1), 1);
    cluster.forEach((p) => (p.lanes = lanes));
    out.push(...cluster);
    cluster = [];
  };
  for (const b of sorted) {
    if (cluster.length && b.start >= clusterEnd) flush();
    const taken = new Set(cluster.filter((p) => p.end > b.start).map((p) => p.lane));
    let lane = 0;
    while (taken.has(lane)) lane++;
    cluster.push({ ...b, lane, lanes: 1 });
    clusterEnd = Math.max(clusterEnd, b.end);
  }
  flush();
  return out;
}
