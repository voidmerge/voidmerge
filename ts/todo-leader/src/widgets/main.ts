import { WidgetVert } from "./vert.js";
import { WidgetHoriz } from "./horiz.js";
import { WidgetTodo } from "./todo.js";
import { WidgetLabel } from "./label.js";
import { WidgetAvatar } from "./avatar.js";
import { Ident } from "../ident.js";

export class WidgetMain extends WidgetVert {
  #header: WidgetHoriz;

  constructor(ident: Ident) {
    super();

    this.#header = new WidgetHoriz();
    this.append(this.#header);
    this.#header.append(new WidgetLabel(ident.short()));
    this.#header.append(new WidgetAvatar(ident.avatarCode()));

    this.append(new WidgetTodo());
    this.append(new WidgetTodo());
    this.append(new WidgetTodo());
    this.append(new WidgetTodo());
    this.append(new WidgetTodo());
  }
}
