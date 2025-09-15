from fastapi import FastAPI
from pydantic import BaseModel
from typing import List, Optional, Dict, Any
import torch

from transformers import AutoModelForCausalLM, AutoTokenizer

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


class ModelHolder:
    def __init__(self):
        self.model_id = None
        self.model = None
        self.tok = None

    def load(self, model_id: str):
        if self.model_id == model_id and self.model is not None:
            return
        self.tok = AutoTokenizer.from_pretrained(model_id, use_fast=True)
        self.model = AutoModelForCausalLM.from_pretrained(model_id)
        device = "cuda" if torch.cuda.is_available() else "cpu"
        self.model.to(device)
        self.model_id = model_id


holder = ModelHolder()


@app.get("/health")
def health():
    return {"status": "ok"}


@app.post("/generate", response_model=GenerateResponse)
def generate(req: GenerateRequest):
    model_id = req.model or _default_model()
    holder.load(model_id)

    if req.seed is not None:
        torch.manual_seed(req.seed)

    tok = holder.tok
    model = holder.model

    inputs = tok(req.prompt, return_tensors="pt")
    inputs = {k: v.to(model.device) for k, v in inputs.items()}

    gen_kwargs = {
        "max_new_tokens": req.max_new_tokens or 512,
        "temperature": req.temperature if req.temperature is not None else 0.7,
        "top_p": req.top_p if req.top_p is not None else 0.95,
        "do_sample": True if req.do_sample is None else req.do_sample,
    }
    if req.top_k is not None:
        gen_kwargs["top_k"] = req.top_k
    if req.repetition_penalty is not None:
        gen_kwargs["repetition_penalty"] = req.repetition_penalty
    if req.eos_token_id is not None:
        gen_kwargs["eos_token_id"] = req.eos_token_id
    if req.pad_token_id is not None:
        gen_kwargs["pad_token_id"] = req.pad_token_id

    with torch.no_grad():
        out = model.generate(**inputs, **gen_kwargs)

    text = tok.decode(out[0], skip_special_tokens=True)

    if text.startswith(req.prompt):
        text = text[len(req.prompt):]

    if req.stop:
        for s in req.stop:
            idx = text.find(s)
            if idx != -1:
                text = text[:idx]
                break

    return GenerateResponse(text=text, usage=None)


def _default_model() -> str:
    import os
    return os.getenv("RAWORC_GPT_MODEL", "gpt-oss:120b")

