const btoaResult = btoa("foo");
const btoaExpected = "Zm9v";
if (btoaResult !== btoaExpected) {
  throw new Error(`btoaResult was not '${btoaExpected}', got: '${btoaResult}'`);
}

const atobResult = atob("Zm9v");
const atobExpected = "foo";
if (atobResult !== atobExpected) {
  throw new Error(`atobResult was not '${atobExpected}', got: '${atobResult}'`);
}
