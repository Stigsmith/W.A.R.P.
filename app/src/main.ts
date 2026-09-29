import { mount } from "svelte";
import App from "./App.svelte";
import "@fontsource/grenze-gotisch/latin-500.css";
import "@fontsource/grenze-gotisch/latin-600.css";
import "@fontsource/grenze-gotisch/latin-700.css";
import "@fontsource/pirata-one/latin-400.css";
import "@fontsource/new-rocker/latin-400.css";
import "@fontsource/metamorphous/latin-400.css";
import "@fontsource/im-fell-english/latin-400.css";
import "@fontsource/im-fell-english/latin-400-italic.css";
import "@fontsource/almendra/latin-400-italic.css";
import "@fontsource/uncial-antiqua/latin-400.css";
import "@fontsource/medievalsharp/latin-400.css";
import "./app.css";

export default mount(App, { target: document.getElementById("app")! });
