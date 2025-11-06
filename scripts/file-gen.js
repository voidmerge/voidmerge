#!/usr/bin/env node

import { XMLParser, XMLBuilder } from "fast-xml-parser";
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

function deleteSodipodi(array) {
  for (const node of array) {
    if ("g" in node) {
      deleteSodipodi(node.g);
    }
    if (":@" in node && "@_sodipodi:nodetypes" in node[":@"]) {
      delete node[":@"]["@_sodipodi:nodetypes"];
    }
  }
}

async function addSvgParts(path) {
  const data = new TextDecoder().decode(await fs.readFile(path));

  const xml = new XMLParser({
    ignoreAttributes: false,
    allowBooleanAttributes: true,
    preserveOrder: true,
  });

  const bld = new XMLBuilder({
    ignoreAttributes: false,
    format: true,
    preserveOrder: true,
  });

  const parsed = (await xml.parse(data))[1].svg;

  for (const node of parsed) {
    if ("g" in node && ":@" in node && "@_inkscape:label" in node[":@"]) {
      deleteSodipodi(node.g);
      const label = node[":@"]["@_inkscape:label"];
      const render = bld.build(node.g);
      assets[`avatar-${label}.svg-part`] = render;
    }
  }
}

async function main() {
  await addSvgParts("ts/todo-leader/src/todo-leader.svg");

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
