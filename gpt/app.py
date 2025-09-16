from fastapi import FastAPI
from fastapi.responses import JSONResponse
from pydantic import BaseModel
from typing import List, Optional, Dict, Any
import torch
import threading
import time
import uuid

from transformers import AutoModelForCausalLM, AutoTokenizer
try:
    # Preferred API for MXFP4 quantization
    from transformers.utils.quantization_config import Mxfp4Config, QuantizationMethod
except Exception:  # pragma: no cover
    Mxfp4Config = None
    QuantizationMethod = None

app = FastAPI()


class GenerateRequest(BaseModel):
    prompt: str
    model: Optional[str] = None
    # Generation params (subset; server ignores unknowns, mirrors internal schema)
    max_new_tokens: Optional[int] = 512
    temperature: Optional[float] = 0.7
    top_p: Optional[float] = 0.95
    top_k: Optional[int] = None
    repetition_penalty: Optional[float] = None
    do_sample: Optional[bool] = True
    eos_token_id: Optional[int] = None
    pad_token_id: Optional[int] = None
    stop: Optional[List[str]] = None
    seed: Optional[int] = None
    request_id: Optional[str] = None
    metadata: Optional[Dict[str, Any]] = None


class GenerateResponse(BaseModel):
    text: str
    usage: Optional[Dict[str, int]] = None

class ReadyResponse(BaseModel):
    status: str
    model: str
    loaded: bool
    loading: bool
    quant_method: Optional[str] = None
    error: Optional[str] = None


class ModelHolder:
    def __init__(self):
        self.model_id = None
        self.model = None
        self.tok = None
        self.loading = False
        self.last_error: Optional[str] = None
        self._lock = threading.Lock()
        self.quant_enforced = True
        self.quant_method: Optional[str] = None

    def load(self, model_id: str):
        # Always load the single supported model (120B)
        model_id = _default_model()
        if self.model_id == model_id and self.model is not None:
            return
        # Actual blocking load
        self.tok = AutoTokenizer.from_pretrained(model_id, use_fast=True)
        load_kwargs: Dict[str, Any] = {}
        # Enforce MXFP4 quantization if supported; fail if it falls back
        if self.quant_enforced:
            if Mxfp4Config is None:
                raise RuntimeError("MXFP4 is required but not available in transformers build")
            qcfg = Mxfp4Config(dequantize=False)
            load_kwargs["quantization_config"] = qcfg
            load_kwargs["device_map"] = "auto"
        # Load model
        self.model = AutoModelForCausalLM.from_pretrained(model_id, **load_kwargs)
        device = "cuda" if torch.cuda.is_available() else "cpu"
        self.model.to(device)
        self.model_id = model_id
        self.last_error = None
        # Validate quantization actually active
        self.quant_method = None
        if self.quant_enforced:
            # transformers attaches hf_quantizer to model when quantized
            hq = getattr(self.model, "hf_quantizer", None)
            if hq is None:
                raise RuntimeError("MXFP4 required but model.hf_quantizer missing (dequantized)")
            qm = getattr(getattr(hq, "quantization_config", None), "quant_method", None)
            dm = getattr(getattr(hq, "quantization_config", None), "dequantize", None)
            self.quant_method = str(qm) if qm is not None else None
            if QuantizationMethod is not None and qm == QuantizationMethod.MXFP4 and dm is False:
                pass  # OK
            else:
                raise RuntimeError("MXFP4 required but quantization fell back (dequantized or different method)")

    def ensure_loaded_async(self, model_id: str):
        """Kick off a background load if needed."""
        with self._lock:
            target = _default_model()
            if (self.model is not None and self.model_id == target) or self.loading:
                return
            self.loading = True

        def _worker():
            try:
                self.load(_default_model())
            except Exception as e:
                self.last_error = str(e)
            finally:
                with self._lock:
                    self.loading = False

        threading.Thread(target=_worker, daemon=True).start()

    def ready_for(self, model_id: str) -> bool:
        target = _default_model()
        return self.model is not None and self.model_id == target


holder = ModelHolder()


@app.get("/health")
def health():
    return {"status": "ok"}

@app.get("/ready", response_model=ReadyResponse)
def ready():
    model_id = _default_model()
    # Optionally kick off background load
    if not holder.ready_for(model_id) and not holder.loading and holder.last_error is None:
        holder.ensure_loaded_async(model_id)
    loaded = holder.ready_for(model_id)
    status = "error" if holder.last_error else ("ready" if loaded else "loading")
    return ReadyResponse(
        status=status,
        model=model_id,
        loaded=loaded,
        loading=holder.loading,
        quant_method=holder.quant_method,
        error=holder.last_error,
    )

@app.on_event("startup")
def enforce_quant_and_eager_load():
    # Enforce MXFP4 at startup; if not available or OOM, exit to avoid running in a bad state
    model_id = _default_model()
    try:
        holder.quant_enforced = True
        holder.load(model_id)
    except Exception as e:
        # Print clear fatal error so container logs show why it exited
        import sys
        print(
            f"FATAL: MXFP4 quantization is required and failed during startup for model {model_id}: {e}",
            file=sys.stderr,
        )
        # Re-raise to make Uvicorn exit with non-zero status
        raise


@app.post("/generate", response_model=GenerateResponse)
def generate(req: GenerateRequest):
    model_id = _default_model()
    # Return quickly with 202 while model is loading
    if not holder.ready_for(model_id):
        holder.ensure_loaded_async(model_id)
        if holder.last_error:
            return JSONResponse(status_code=503, content={
                "status": "error",
                "error": holder.last_error,
                "hint": "MXFP4 quantization is required. Ensure CUDA GPU (cc>=7.5), triton>=3.4, and kernels package are available."
            })
        if not holder.ready_for(model_id):
            return JSONResponse(status_code=202, content={
                "status": "loading",
                "model": model_id,
                "error": holder.last_error,
            })

    if req.seed is not None:
        torch.manual_seed(req.seed)

    tok = holder.tok
    model = holder.model

    inputs = tok(req.prompt, return_tensors="pt")
    inputs = {k: v.to(model.device) for k, v in inputs.items()}
    # Provide attention_mask explicitly for decoder-only models
    if "attention_mask" not in inputs:
        inputs["attention_mask"] = torch.ones_like(inputs["input_ids"]).to(model.device)

    # Single-pass generation (no fallback)
    model.eval()
    t0 = time.perf_counter()
    try:
        with torch.inference_mode():
            out = model.generate(
                input_ids=inputs["input_ids"],
                attention_mask=inputs["attention_mask"],
                max_new_tokens=(req.max_new_tokens or 128),
            )
    except Exception as e:
        return JSONResponse(status_code=503, content={
            "status": "error",
            "error": f"generation failed: {e}",
        })
    gen_ms = (time.perf_counter() - t0) * 1000.0

    # Decode only newly generated tokens and preserve special tokens
    try:
        input_len = int(inputs["input_ids"].shape[1])
        gen_ids = out[0][input_len:]
        text = tok.decode(gen_ids, skip_special_tokens=False)
    except Exception as e:
        return JSONResponse(status_code=503, content={
            "status": "error",
            "error": f"decode failed: {e}",
        })

    # Do not trim or normalize the generated text on the server side.

    # Compute token usage
    prompt_tokens = int(inputs["input_ids"].shape[1])
    total_tokens = int(out.shape[1])
    completion_tokens = max(0, total_tokens - prompt_tokens)

    # Log concise generation summary to stdout (captured by container logs)
    rid = req.request_id or str(uuid.uuid4())
    try:
        print(
            f"GEN req_id={rid} model={holder.model_id} prompt_tokens={prompt_tokens} completion_tokens={completion_tokens} total_tokens={prompt_tokens + completion_tokens} gen_ms={gen_ms:.1f}",
            flush=True,
        )
    except Exception:
        pass

    return GenerateResponse(
        text=text,
        usage={
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens,
            "gen_ms": int(gen_ms),
        },
    )


def _default_model() -> str:
    import os
    # Use the canonical HF repo id for the single supported model
    return os.getenv("RAWORC_GPT_MODEL", "openai/gpt-oss-120b")
