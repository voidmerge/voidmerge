import { MainState, getWeekId } from "./state.js";
import { Ident } from "./ident.js";
import { WidgetPage, WidgetLoading, WidgetMain } from "./widgets.js";
import { fnCall } from "@voidmerge/voidmerge-client";

const page = new WidgetPage();
page.setChild(new WidgetLoading());

function debounce(cb: () => void): () => void {
  let timer: any = undefined;
  return () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      clearTimeout(timer);
      cb();
    }, 1000);
  };
}

const STATE = "TodoLeaderState";

function loadState(): MainState {
  const raw = JSON.parse(localStorage.getItem(STATE) || "{}");
  const out: MainState = {
    promoted: false,
    weekId: "",
    starCount: 0,
    league: 1,
    leagueData: { state: "no-data" },
    lastLeagueUpdate: 0,
    todo: [],
  };

  if (
    raw &&
    typeof raw === "object" &&
    Array.isArray(raw.todo) &&
    typeof raw.promoted === "boolean" &&
    typeof raw.weekId === "string" &&
    typeof raw.starCount === "number" &&
    typeof raw.league === "number" &&
    typeof raw.leagueData === "object" &&
    typeof raw.lastLeagueUpdate === "number"
  ) {
    out.promoted = raw.promoted;
    out.weekId = raw.weekId;
    out.starCount = raw.starCount;
    out.league = raw.league;
    out.leagueData = raw.leagueData;
    out.lastLeagueUpdate = raw.lastLeagueUpdate;
    for (const todo of raw.todo) {
      if (
        typeof todo === "object" &&
        typeof todo.stars === "number" &&
        typeof todo.todo === "string"
      ) {
        out.todo.push({ stars: todo.stars, todo: todo.todo });
      }
    }
  }

  while (out.todo.length < 5) {
    out.todo.push({ stars: 1, todo: "" });
  }

  if (out.weekId.length < 1) {
    out.weekId = getWeekId();
  }

  const curWeekId = getWeekId();
  if (out.weekId !== curWeekId) {
    out.weekId = curWeekId;
    const tgt = out.league * 5;
    if (out.starCount >= tgt) {
      out.league += 1;
      out.promoted = true;
    }
    if (out.starCount < tgt - 5) {
      out.league -= 1;
    }
    if (out.league < 1) {
      out.league = 1;
    }
    out.starCount = 0;
  }

  return out;
}

async function checkLeague(): Promise<boolean> {
  const now = Date.now();
  if (now - state.lastLeagueUpdate < 1000 * 60 * 5) {
    return false;
  }
  const u = new URL(globalThis.location.href);
  const res = await fnCall({
    url: u.origin,
    ctx: u.pathname.split("/")[1],
    path: "agg",
  });
  state.leagueData = JSON.parse(new TextDecoder().decode(res.body));
  if (state.leagueData.state === "success") {
    state.lastLeagueUpdate = now;
  }
  saveState();
  console.log("LEAGUE RESULT", state.leagueData);
  return true;
}

let ident: boolean | Ident = false;
const state = loadState();
let lastPublishedPath = "";
let lastPublishedStarCount = 0;
let wantPublishPath = "";
let wantPublishData: any[] = [];
let wantPublishStarCount = 0;

function saveState() {
  localStorage.setItem(STATE, JSON.stringify(state));
}

async function prepPublish() {
  if (!(ident instanceof Ident)) {
    return;
  }

  const { path, data } = await ident.sign({
    league: state.league,
    stars: state.starCount,
    weekId: state.weekId,
  });

  wantPublishPath = path;
  wantPublishData = data;
  wantPublishStarCount = state.starCount;
}

async function tryPublish() {
  if (
    wantPublishPath === lastPublishedPath &&
    wantPublishStarCount === lastPublishedStarCount
  ) {
    return;
  }
  wantPublishData.unshift(wantPublishPath);

  console.log("PUBLISH", wantPublishData);

  const u = new URL(globalThis.location.href);

  try {
    const res = await fnCall({
      url: u.origin,
      ctx: u.pathname.split("/")[1],
      path: "publish",
      body: new TextEncoder().encode(JSON.stringify(wantPublishData)),
    });

    console.log("PUBLISH RESULT", new TextDecoder().decode(res.body));

    lastPublishedPath = wantPublishPath;
    lastPublishedStarCount = wantPublishStarCount;
  } catch (err: any) {
    console.error("PUBLISH ERROR", err);
  }
}

saveState();

let main: undefined | WidgetMain = undefined;

async function sync() {
  if (ident === true) {
    // already loading, exit this
    return;
  }

  if (!ident) {
    ident = true;
    let tmp = await Ident.load();
    if (!tmp) {
      tmp = await Ident.random();
      tmp.store();
    }
    ident = tmp;

    console.log("loaded", ident.debug(), state);

    // publish current state asap
    prepPublish();
  }

  if (!main) {
    main = new WidgetMain(page, ident, state);

    main.setUpdate(
      debounce(() => {
        saveState();
        prepPublish();
      }),
    );
  }

  await tryPublish();
  if (await checkLeague()) {
    main.render();
  }
}

let lastSync: number = 0;
function checkAnim(curTimestamp: number) {
  requestAnimationFrame(checkAnim);

  // re-check state roughly every 10 seconds
  if (curTimestamp - lastSync > 1000 * 10) {
    lastSync = curTimestamp;
    setTimeout(sync, 0);
  }
}
setInterval(() => {
  requestAnimationFrame(checkAnim);
}, 10000);
requestAnimationFrame(checkAnim);
sync();
