import { WidgetPage } from "./page.js";
import { WidgetVert } from "./vert.js";
import { WidgetHoriz } from "./horiz.js";
import { WidgetTodo } from "./todo.js";
import { WidgetLabel } from "./label.js";
import { WidgetAvatar } from "./avatar.js";
import { WidgetLeague } from "./league.js";
import { Emoji, WidgetEmoji } from "./emoji.js";
import type { MainState } from "../state.ts";
import { Ident } from "../ident.js";

export class WidgetMain extends WidgetVert {
  #page: WidgetPage;
  #update: () => void;
  #state: MainState;
  #ident: Ident;
  #header: WidgetHoriz;
  #avatar: WidgetAvatar;
  #weekLabel: WidgetLabel;
  #starLabel: WidgetLabel;
  #leagueLabel: WidgetLabel;
  #todo: WidgetTodo[];
  #league: WidgetLeague;

  constructor(page: WidgetPage, ident: Ident, state: MainState) {
    super();

    this.#page = page;
    this.#page.setChild(this);

    this.#update = () => {};
    this.#state = state;

    this.#ident = ident;
    this.#todo = [];

    this.#header = new WidgetHoriz();
    this.append(this.#header);
    this.#avatar = new WidgetAvatar(ident.avatarCode());
    this.#avatar.handleClick(() => {
      this.randomizeAvatar();
    });
    this.#header.append(this.#avatar);
    this.#header.append(new WidgetLabel(ident.short()));

    const cal = new WidgetEmoji(Emoji.calendar);
    cal.handleClick(() => {
      this.#page.setChild(this.#league);
      this.#state.promoted = false;
      this.#update();
    });
    this.#header.append(cal);
    this.#weekLabel = new WidgetLabel("-");
    this.#header.append(this.#weekLabel);

    const star = new WidgetEmoji(Emoji.star);
    star.handleClick(() => {
      this.#page.setChild(this.#league);
      this.#state.promoted = false;
      this.#update();
    });
    this.#header.append(star);
    this.#starLabel = new WidgetLabel("0");
    this.#header.append(this.#starLabel);

    const trophy = new WidgetEmoji(Emoji.trophy);
    trophy.handleClick(() => {
      this.#page.setChild(this.#league);
      this.#state.promoted = false;
      this.#update();
    });
    this.#header.append(trophy);
    this.#leagueLabel = new WidgetLabel("1");
    this.#header.append(this.#leagueLabel);

    for (const todoState of this.#state.todo) {
      const todo = new WidgetTodo(this.#state, todoState);
      this.#todo.push(todo);
      this.append(todo);
    }

    this.#league = new WidgetLeague(page, this, ident, state);

    this.render();
  }

  randomizeAvatar() {
    this.#ident.randomizeAvatar();
    this.#avatar.setAvatarCode(this.#ident.avatarCode());
  }

  render() {
    this.#league.render();

    for (const todo of this.#todo) {
      todo.render();
    }
    this.#weekLabel.set(this.#state.weekId.split("-")[1]);

    const tot = this.#state.league * 5;
    const cur = this.#state.starCount;
    let starLabel = `${cur}/${tot}`;
    if (cur >= tot) {
      starLabel += "!";
    }
    this.#starLabel.set(starLabel);

    let leagueLabel = this.#state.league.toString();
    if (this.#state.promoted) {
      const prev = this.#state.league - 1;
      leagueLabel = `${prev}->${leagueLabel}!`;
    }
    this.#leagueLabel.set(leagueLabel);
  }

  setUpdate(update: () => void) {
    this.#update = () => {
      this.render();
      update();
    };
    this.#league.setUpdate(update);
    for (const todo of this.#todo) {
      todo.setUpdate(() => {
        this.render();
        update();
      });
    }
  }
}
