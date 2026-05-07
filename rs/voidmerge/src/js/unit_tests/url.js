const expected = "http://127.0.0.1:123/path?a=1&b=2#test";

const t1 = new URL(expected);
const t2 = new URL("https://stub");
t2.protocol = "http://";
t2.hostname = "127.0.0.1";
t2.port = 123;
t2.pathname = "path";
t2.search = "a=1&b=2";
t2.hash = "test";

if (t1.toString() !== expected) {
  throw new Error(`URL t1 expected '${expected}', got: '${t1.toString()}'`);
}

if (t2.toString() !== expected) {
  throw new Error(`URL t2 expected '${expected}', got: '${t2.toString()}'`);
}
