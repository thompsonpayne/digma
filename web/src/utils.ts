import type { RgbaColor } from "./editorTypes";

export const hexToRgbaColor = (hex: string): RgbaColor | null => {
	const normalized = hex.trim();

	if (!/^#[0-9a-fA-F]{6}$/.test(normalized)) {
		return null;
	}
	const r = Number.parseInt(normalized.slice(1, 3), 16) / 255;
	const g = Number.parseInt(normalized.slice(3, 5), 16) / 255;
	const b = Number.parseInt(normalized.slice(5, 7), 16) / 255;

	return { r, g, b, a: 1.0 };
};
