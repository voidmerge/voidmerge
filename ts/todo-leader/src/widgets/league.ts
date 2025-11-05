import { WidgetPage } from "./page.js";
import { WidgetMain } from "./main.js";
import { WidgetHoriz } from "./horiz.js";
import { WidgetAvatar } from "./avatar.js";
import { WidgetLabel } from "./label.js";
import { Emoji, WidgetEmoji } from "./emoji.js";
import type { MainState } from "../state.ts";
import { Ident } from "../ident.js";

export class WidgetLeague extends WidgetHoriz {
  #page: WidgetPage;
  #main: WidgetMain;
  #update: () => void;
  #state: MainState;
  #ident: Ident;
  #back: WidgetEmoji;

  constructor(
    page: WidgetPage,
    main: WidgetMain,
    ident: Ident,
    state: MainState,
  ) {
    super();

    this.setWrap(true);

    this.#page = page;
    this.#main = main;

    this.#update = () => {};
    this.#state = state;

    this.#ident = ident;

    this.#back = new WidgetEmoji(Emoji.back);
    this.#back.handleClick(() => this.#page.setChild(this.#main));

    this.render();
  }

  render() {
    this.clear();
    this.append(this.#back);

    const league = this.#state.leagueData.leagues[this.#state.league - 1];
    for (const [id, stars, avatarCode] of league) {
      const avatar = new WidgetAvatar(avatarCode);
      this.append(avatar);
    }
  }

  setUpdate(update: () => void) {
    this.#update = () => {
      this.render();
      update();
    };
  }
}
