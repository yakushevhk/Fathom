/** Client-side image compressor: converts large PNGs/JPEGs to optimized WebP
 * before upload, saving up to 80% vision tokens and bandwidth. */
export async function compressImageForUpload(file: File, maxDimension = 1920, quality = 0.82): Promise<File> {
  // If not an image or SVG/GIF (which may be animated), return original
  if (!file.type.startsWith("image/") || file.type === "image/gif" || file.type === "image/svg+xml") {
    return file;
  }

  // If already small (< 300 KB), skip compression
  if (file.size < 300 * 1024) {
    return file;
  }

  // Ensure DOM/browser environment supports required APIs
  if (
    typeof window === "undefined" ||
    typeof document === "undefined" ||
    typeof URL === "undefined" ||
    typeof URL.createObjectURL !== "function"
  ) {
    return file;
  }

  return new Promise((resolve) => {
    let settled = false;
    const finish = (result: File) => {
      if (settled) return;
      settled = true;
      resolve(result);
    };

    let url = "";
    try {
      url = URL.createObjectURL(file);
    } catch {
      finish(file);
      return;
    }

    const img = new Image();

    img.onload = () => {
      try {
        URL.revokeObjectURL(url);
      } catch {
        // Ignore revoke errors
      }

      let { width, height } = img;
      if (!width || !height) {
        finish(file);
        return;
      }

      if (width > maxDimension || height > maxDimension) {
        if (width > height) {
          height = Math.round((height * maxDimension) / width);
          width = maxDimension;
        } else {
          width = Math.round((width * maxDimension) / height);
          height = maxDimension;
        }
      }

      let canvas: HTMLCanvasElement;
      let ctx: CanvasRenderingContext2D | null;
      try {
        canvas = document.createElement("canvas");
        canvas.width = width;
        canvas.height = height;
        ctx = canvas.getContext("2d");
      } catch {
        finish(file);
        return;
      }

      if (!ctx) {
        finish(file);
        return;
      }

      try {
        ctx.drawImage(img, 0, 0, width, height);
      } catch {
        finish(file);
        return;
      }

      try {
        canvas.toBlob(
          (blob) => {
            if (!blob || blob.size >= file.size) {
              finish(file);
              return;
            }
            const newName = file.name.includes(".")
              ? file.name.replace(/\.[^.]+$/, ".webp")
              : `${file.name}.webp`;
            const compressed = new File([blob], newName, {
              type: "image/webp",
              lastModified: Date.now(),
            });
            finish(compressed);
          },
          "image/webp",
          quality,
        );
      } catch {
        finish(file);
      }
    };

    img.onerror = () => {
      try {
        URL.revokeObjectURL(url);
      } catch {
        // Ignore revoke errors
      }
      finish(file);
    };

    img.src = url;
  });
}
