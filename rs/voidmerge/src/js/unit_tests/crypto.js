const MSG = new TextEncoder().encode('hello world');
const MSG2 = new TextEncoder().encode('bad message');

// getRandomValues
const randNotExpected = new Uint8Array(16).toString();
const rand = crypto.getRandomValues(new Uint8Array(16));
if (rand === randNotExpected) {
  throw new Error(`expected random data, got: ${rand}`);
}

// sha256
const hash = Array.from(new Uint8Array(await crypto.subtle.digest(
  'SHA-256',
  MSG,
))).map(b => b.toString(16).padStart(2, '0')).join('');
const hashExpected = 'b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9';
if (hash !== hashExpected) {
  throw new Error(`sha256 expected '${hashExpected}', got: '${hash}'`);
}

// p256
const p256keys = await crypto.subtle.generateKey(
  { name: 'ECDSA', namedCurve: 'P-256' },
  true,
  ['sign', 'verify'],
);
const sig = await crypto.subtle.sign(
  { name: 'ECDSA', hash: { name: 'SHA-256' } },
  p256keys.privateKey,
  MSG,
);
const isValid = await crypto.subtle.verify(
  { name: 'ECDSA', hash: { name: 'SHA-256' } },
  p256keys.publicKey,
  sig,
  MSG,
);
if (!isValid) {
  throw new Error('invalid p256 signature');
}
const isValid2 = await crypto.subtle.verify(
  { name: 'ECDSA', hash: { name: 'SHA-256' } },
  p256keys.publicKey,
  sig,
  MSG2,
);
if (isValid2) {
  throw new Error('unexpected valid p256 signature');
}

// aes 256 gcm
const aesKey = await crypto.subtle.generateKey(
  { name: 'AES-GCM', length: 256 },
  true,
  ['encrypt', 'decrypt'],
);
const aesIv = crypto.getRandomValues(new Uint8Array(12));
const aesCipher = await crypto.subtle.encrypt(
  { name: 'AES-GCM', iv: aesIv },
  aesKey,
  MSG,
);
const aesPlain = await crypto.subtle.decrypt(
  { name: 'AES-GCM', iv: aesIv },
  aesKey,
  aesCipher,
);
const aesPlainStr = new TextDecoder().decode(new Uint8Array(aesPlain));
if (aesPlainStr !== 'hello world') {
  throw new Error(`expected: 'hello world', got: '${aesPlainStr}'`);
}
