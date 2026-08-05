/**
 * The three sections that have nothing to show yet.
 *
 * Dictionary, History and Stats all read from the database that M5 builds.
 * Rather than mock them up with invented rows, each one says plainly what it
 * will hold and what it is waiting for — an empty state that is true beats a
 * screenshot that is not.
 */

export function Pending({
  title,
  what,
  waiting,
}: {
  title: string;
  what: string;
  waiting: string;
}) {
  return (
    <div className="pending">
      <h2>{title}</h2>
      <p className="pending__what">{what}</p>
      <p className="pending__waiting">{waiting}</p>
    </div>
  );
}

export const DICTIONARY = {
  title: "Teach Klar your words",
  what: "Names, product names and jargon the model will otherwise guess at. Add a term once and Klar spells it correctly from then on.",
  waiting:
    "Waiting on storage. Terms will bias recognition through whisper's initial prompt and correct anything that still comes out wrong.",
};

export const HISTORY = {
  title: "History",
  what: "Every dictation, kept on this machine. Nothing is uploaded, and clearing it is instant and permanent.",
  waiting: "Waiting on storage.",
};

export const STATS = {
  title: "Stats",
  what: "Words dictated, time saved against your measured typing speed, and where the text went.",
  waiting: "Waiting on storage.",
};
