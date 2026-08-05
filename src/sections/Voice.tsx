import { Row, Segmented, Select } from "../components/Row";
import type { Cleanup, Device, ModelStatus, Processing, Settings } from "../lib/settings";
import { LANGUAGES } from "../lib/settings";

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
      <Row
        label="Processing"
        hint="Local never leaves the machine. Cloud is faster on a slow computer and is not."
      >
        <Segmented<Processing>
          value={settings.processing}
          options={[
            { value: "local", label: "On this PC" },
            { value: "cloud", label: "Klar cloud" },
          ]}
          onChange={(processing) => {
            onChange({ ...settings, processing });
          }}
        />
      </Row>

      <Row
        label="Model"
        hint={
          current
            ? `${current.size} — ${current.installed ? "installed" : "not downloaded"}`
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

      <Row label="Cleanup strength" hint="How much Klar rewrites what you said.">
        <Segmented<Cleanup>
          value={settings.cleanup}
          options={[
            { value: "verbatim", label: "Verbatim" },
            { value: "light", label: "Light" },
            { value: "balanced", label: "Balanced" },
            { value: "heavy", label: "Heavy" },
          ]}
          onChange={(cleanup) => {
            onChange({ ...settings, cleanup });
          }}
        />
      </Row>

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
          An illustration. The polish stage arrives in the next milestone; until then Klar inserts
          the transcript as recognised.
        </p>
      </div>
    </>
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
