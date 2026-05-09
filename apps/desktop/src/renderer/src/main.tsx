import { render } from "solid-js/web";
import { App } from "./App";
import "./styles.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("APKRunner renderer root element is missing.");
}

render(() => <App />, root);
