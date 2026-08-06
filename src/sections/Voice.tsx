import { useEffect, useState } from "react";
import { Row, Segmented, Select } from "../components/Row";
import type {
  Acceleration,
  Accuracy,
  Cleanup,
  Device,
  ModelStatus,
  Settings,
} from "../lib/settings";
import { LANGUAGES, acceleration, polishStatus, type PolishStatus } from "../lib/settings";

/** The example from the design, showing what each cleanup strength does. */
const SAID =
  "so I guess we should um push the review to Thursday — no, Friday, and uh I'll write up the notes after";

const TYPED: Record<Cleanup, string> = {
  verbatim: SAID,
  light:
    "So I guess we should push the review to Thursday — no, Friday, and I'll write up the notes after.",
  balanced: "We should push the review to Friday, and I'll write up the notes after.",
  heavy: "Review moved to Friday. I'll write up the notes.",
};

export function Voice({
  settings,
  models,
  devices,
  level,
  onChange,
}: {
  settings: Settings;
  models: ModelStatus[];
  devices: Device[];
  level: number;
  onChange: (next: Settings) => void;
}) {
  const speech = models.filter((model) => model.id !== "silero-vad");
  const current = models.find((model) => model.id === settings.model);

  return (
    <>
      <Processing />

      <Row
        label="Model"
        hint={
          current
            ? `${current.summary} ${current.size}, ${current.installed ? "installed" : "not downloaded"}.`
            : "One model is loaded at a time."
        }
      >
        <Select
          value={settings.model}
          options={speech.map((model) => ({
            value: model.id,
            label: model.installed ? model.id : `${model.id} (not downloaded)`,
          }))}
          onChange={(model) => {
            onChange({ ...settings, model });
          }}
        />
      </Row>

      <Row
        label="Recognition"
        hint="Accurate weighs several candidates for each word instead of taking the first. It costs time on every dictation and buys back the words that sound like other words — names, jargon, anything the model has not seen before. Measured at 260 ms against a 500 ms budget on Fast, so there is room."
      >
        <Segmented<Accuracy>
          value={settings.accuracy}
          options={[
            { value: "fast", label: "Fast" },
            { value: "accurate", label: "Accurate" },
          ]}
          onChange={(accuracy) => {
            onChange({ ...settings, accuracy });
          }}
        />
      </Row>

      <Row label="Language" hint="Pinning it saves the detection pass — about 100 ms a dictation.">
        <Select
          value={settings.language}
          options={LANGUAGES.map(({ code, label }) => ({ value: code, label }))}
          onChange={(language) => {
            onChange({ ...settings, language });
          }}
        />
      </Row>

      <Row label="Microphone" hint="Input level, live.">
        <div className="mic">
          <Select
            value={settings.microphone}
            options={[
              { value: null, label: "System default" },
              ...devices.map((device) => ({ value: device.name, label: device.name })),
            ]}
            onChange={(microphone) => {
              onChange({ ...settings, microphone });
            }}
          />
          <Meter level={level} />
        </div>
      </Row>

      <Row
        label="Cleanup strength"
        hint="How much Klar rewrites what you said. Verbatim switches the polish stage off entirely — the transcript is inserted exactly as recognised, and no language model sees it."
      >
        <Segmented<Cleanup>
          value={settings.cleanup}
          options={[
            { value: "verbatim", label: "Verbatim — off" },
            { value: "light", label: "Light" },
            { value: "balanced", label: "Balanced" },
            { value: "heavy", label: "Heavy" },
          ]}
          onChange={(cleanup) => {
            onChange({ ...settings, cleanup });
          }}
        />
      </Row>

      {settings.cleanup !== "verbatim" && <PolishModel settings={settings} onChange={onChange} />}

      <div className="example">
        <div className="example__row">
          <div className="label">Said</div>
          <p className="example__said">{SAID}</p>
        </div>
        <div className="example__row">
          <div className="label">Typed</div>
          <p className="example__typed">{TYPED[settings.cleanup]}</p>
        </div>
        <p className="example__note">
          An illustration of each setting, not a recorded result. What a model actually returns
          depends on which one you point Klar at.
        </p>
      </div>
    </>
  );
}

/**
 * Where speech is processed, and on what.
 *
 * The row used to read "On this PC" and stop there, which is true and useless:
 * an installer built for CUDA runs perfectly on a machine with an AMD card,
 * finds no CUDA device, falls back to the CPU, and every dictation takes
 * several seconds with nothing anywhere saying why. Naming the device turns
 * that into something a person can act on.
 */
function Processing() {
  const [state, setState] = useState<Acceleration | null>(null);

  useEffect(() => {
    let cancelled = false;
    acceleration()
      .then((probed) => {
        if (!cancelled) setState(probed);
      })
      .catch(() => {
        // Not knowing is not worth an error in a settings window — the row
        // falls back to what it said before there was a probe.
        if (!cancelled) setState(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const hint =
    state?.warning ??
    "Speech is recognised on this machine and never leaves it. A cloud option is planned for computers without a usable GPU; it is not built, so it is not offered.";

  return (
    <Row label="Processing" hint={hint} alert={state?.warning != null}>
      <span className="figure">{state ? `On this PC — ${state.summary}` : "On this PC"}</span>
    </Row>
  );
}

/** The live input meter. Only moves while a dictation is running — there is no
 * second audio stream open just to draw this. */
function Meter({ level }: { level: number }) {
  const filled = Math.round(Math.min(1, Math.sqrt(level)) * 16);
  return (
    <div className="meter" aria-label="input level">
      {Array.from({ length: 16 }, (_, index) => (
        <span key={index} className="meter__cell" data-on={index < filled ? "true" : "false"} />
      ))}
    </div>
  );
}

/**
 * Which local model does the polishing.
 *
 * No default name is offered. What a machine has pulled is what works, the
 * useful sizes change every few months, and naming one here would send somebody
 * after a download that may be the wrong one. So the list is whatever Ollama
 * reports, and when it reports nothing the row says why rather than showing an
 * empty menu.
 */
function PolishModel({
  settings,
  onChange,
}: {
  settings: Settings;
  onChange: (next: Settings) => void;
}) {
  const [status, setStatus] = useState<PolishStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    polishStatus()
      .then((reported) => {
        if (!cancelled) setStatus(reported);
      })
      .catch(() => {
        if (!cancelled)
          setStatus({ reachable: false, endpoint: settings.polishEndpoint, models: [] });
      });
    return () => {
      cancelled = true;
    };
  }, [settings.polishEndpoint]);

  if (status === null) {
    return <Row label="Polish model" hint="Asking the local model server…" children={null} />;
  }

  if (!status.reachable) {
    return (
      <Row
        label="Polish model"
        hint={`Nothing is answering at ${status.endpoint}. Install Ollama and pull a model to switch this on — dictation works without it and inserts the transcript as recognised.`}
      >
        <span className="figure">Off</span>
      </Row>
    );
  }

  return (
    <Row
      label="Polish model"
      hint={
        settings.polishModel
          ? "Runs on this machine. Nothing is sent anywhere."
          : "Pick one to switch the polish stage on."
      }
    >
      <Select
        value={settings.polishModel}
        options={[
          { value: "", label: "None — insert as recognised" },
          ...status.models.map((model) => ({ value: model, label: model })),
        ]}
        onChange={(polishModel) => {
          onChange({ ...settings, polishModel });
        }}
      />
    </Row>
  );
}
