#!/usr/bin/env node

import fs from "node:fs/promises";
import { minify as htmlMinifyLib } from "html-minifier";
import CleanCss from "clean-css";

function htmlMinify(data) {
  return htmlMinifyLib(data, {
    collapseWhitespace: true,
    removeComments: true,
    removeRedundantAttributes: true,
    removeTagWhitespace: true,
    minifyCss: true,
    minifyJs: true,
    maxLineLength: 120,
  });
}

function cssMinify(data) {
  return new CleanCss({}).minify(data).styles;
}

const assets = {};

async function addAsset(path, as, minify) {
  let data = new TextDecoder().decode(await fs.readFile(path));
  if (minify) {
    data = minify(data);
  }
  assets[as] = data;
}

async function main() {
  await addAsset("ts/todo-leader/src/index.html", "index.html", htmlMinify);
  await addAsset("ts/todo-leader/src/index.css", "index.css", cssMinify);
  await addAsset("ts/todo-leader/dist/bundle-todo-client.js", "index.js");
  await addAsset("book/theme/favicon.svg", "favicon.svg");

  const content = JSON.stringify(assets, null, 2);

  await fs.writeFile(
    "ts/todo-leader/src/assets.ts",
    `/**
 * This file is auto-generated, do not edit directly!
 */
const out: { [k: string]: string } = ${content};
export default out;\n`,
  );
}

main().then(
  () => {},
  (err) => {
    console.error(err);
  },
);
