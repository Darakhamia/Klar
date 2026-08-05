import React from "react";
import ReactDOM from "react-dom/client";
import { Overlay } from "./Overlay";
import { useEngine } from "./useEngine";

function App() {
  return <Overlay engine={useEngine()} />;
}

const root = document.getElementById("overlay");
if (!root) throw new Error("overlay.html is missing #overlay");

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
