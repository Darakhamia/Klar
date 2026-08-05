import React from "react";
import ReactDOM from "react-dom/client";
import { Onboarding } from "./Onboarding";
import "../styles/global.css";

const root = document.getElementById("onboarding");
if (!root) throw new Error("onboarding.html is missing #onboarding");

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <Onboarding />
  </React.StrictMode>,
);
