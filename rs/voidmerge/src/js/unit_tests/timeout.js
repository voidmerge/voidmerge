let timeout1Complete = false;
let timeout2Complete = false;

setTimeout(() => {
  timeout1Complete = true;
}, 50);

const id = setTimeout(() => {
  timeout2Complete = true;
}, 50);

await new Promise((res) => setTimeout(res, 10));

clearTimeout(id);

await new Promise((res) => setTimeout(res, 60));

if (timeout1Complete === false) {
  throw new Error('timeout1Complete was not true, setTimeout failed');
}

if (timeout2Complete === true) {
  throw new Error('timeout2Complete was true, clearTimeout failed');
}
