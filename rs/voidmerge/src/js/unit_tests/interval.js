let interval1Count = 0;
let interval2Count = 0;

const id1 = setInterval(() => {
  interval1Count += 1;
}, 10);

const id2 = setInterval(() => {
  interval2Count += 1;
}, 10);

await new Promise((res) => setTimeout(res, 15));

clearInterval(id2);

await new Promise((res) => setTimeout(res, 60));

clearInterval(id1);

if (interval1Count < 4) {
  throw new Error(`expected interval1Count > 3, got: ${interval1Count}`);
}

if (interval2Count > 3) {
  throw new Error(`expected interval2Count <= 3, got: ${interval2Count}`);
}
