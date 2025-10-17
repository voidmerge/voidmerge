import type { MainState, TodoState } from "../state.ts";
import { WidgetHoriz } from "./horiz.js";
import { Emoji, WidgetEmoji } from "./emoji.js";
import { WidgetText } from "./text.js";

export class WidgetTodo extends WidgetHoriz {
  #state: MainState;
  #todoState: TodoState;
  #stars: WidgetEmoji[];
  #text: WidgetText;
  #check: WidgetEmoji;
  #update: () => void;

  constructor(state: MainState, todoState: TodoState) {
    super();

    this.#state = state;
    this.#todoState = todoState;

    this.#update = () => {};

    this.#stars = [];

    for (let i = 0; i < 5; ++i) {
      const emoji = new WidgetEmoji(Emoji.circleBlack);
      const clickCount = i + 1;
      emoji.handleClick(() => {
        this.setStars(clickCount);
      });
      this.append(emoji);
      this.#stars.push(emoji);
    }

    this.#text = new WidgetText(this.#todoState.todo);
    this.append(this.#text);

    this.#check = new WidgetEmoji(Emoji.check);
    this.append(this.#check);
    this.#check.handleClick(() => this.check());

    this.render();
  }

  setUpdate(update: () => void) {
    this.#update = update;
    this.#text.setUpdate(() => {
      this.#todoState.todo = this.#text.get();
      update();
    });
  }

  render() {
    this.#text.set(this.#todoState.todo);
    let idx = 0;
    for (const star of this.#stars) {
      ++idx;
      if (idx <= this.#todoState.stars) {
        star.set(Emoji.star);
      } else {
        star.set(Emoji.circleBlack);
      }
    }
  }

  check() {
    this.#state.starCount += this.#todoState.stars;
    this.#todoState.todo = "";
    this.#todoState.stars = 1;
    this.render();
    this.#update();
  }

  setStars(count: number) {
    this.#todoState.stars = count;
    this.render();
    this.#update();
  }
}
